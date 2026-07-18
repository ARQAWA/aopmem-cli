//! Exact redaction for explicitly tagged test-secret values.
//!
//! The tag is an operational-memory anchor, not a detector. Callers load one
//! bounded value set from one SQLite read snapshot and reuse it for the whole
//! write, report, UI response, or export.

use crate::storage;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, TransactionBehavior};
use std::collections::BTreeSet;
use thiserror::Error;

pub const TEST_SECRET_TAG: &str = "sensitivity:test_secret";
pub const TEST_SECRET_REDACTION_MARKER: &str = "<TEST_SECRET_REDACTED>";

const MAX_TAGGED_VALUES: usize = 1_024;
const MAX_TAGGED_TOTAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_REDACTION_EXPANSION_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TaggedValueRedactionError {
    #[error("tagged-value redaction source could not be read")]
    ReadFailed,
    #[error("tagged-value redaction source contains an invalid value")]
    InvalidValue,
    #[error("tagged-value redaction source exceeds its bounded size")]
    SourceLimit,
    #[error("tagged-value redaction output exceeds its bounded size")]
    OutputLimit,
}

/// One deterministic, bounded set of exact values loaded from node bodies.
#[derive(Debug, Clone, Default)]
pub struct TaggedValueRedactor {
    values: Vec<Box<[u8]>>,
    by_first_byte: Vec<Vec<usize>>,
    json_escaped_values: Vec<Box<[u8]>>,
    json_escaped_by_first_byte: Vec<Vec<usize>>,
}

impl TaggedValueRedactor {
    /// Opens one deferred operational read transaction and loads the value set.
    pub fn load_workspace(
        workspace_paths: &storage::WorkspacePaths,
    ) -> Result<Self, TaggedValueRedactionError> {
        let mut connection = storage::open_workspace_db_read_only(workspace_paths)
            .map_err(|_| TaggedValueRedactionError::ReadFailed)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|_| TaggedValueRedactionError::ReadFailed)?;
        let redactor = Self::load(&transaction)?;
        transaction
            .commit()
            .map_err(|_| TaggedValueRedactionError::ReadFailed)?;
        Ok(redactor)
    }

    /// Loads exact tagged values with one join query in the caller's snapshot.
    ///
    /// Values are deduplicated, then ordered longest-first and by raw byte
    /// order. A tagged node without a valid non-empty text body fails closed.
    pub fn load(connection: &Connection) -> Result<Self, TaggedValueRedactionError> {
        let mut statement = connection
            .prepare(
                "SELECT nodes.body
                 FROM tags
                 JOIN nodes ON nodes.id = tags.node_id
                 WHERE tags.tag = ?1 COLLATE BINARY
                 ORDER BY tags.node_id ASC, tags.id ASC",
            )
            .map_err(|_| TaggedValueRedactionError::ReadFailed)?;
        let mut rows = statement
            .query([TEST_SECRET_TAG])
            .map_err(|_| TaggedValueRedactionError::ReadFailed)?;
        let mut unique = BTreeSet::new();
        let mut total_bytes = 0_usize;

        while let Some(row) = rows
            .next()
            .map_err(|_| TaggedValueRedactionError::ReadFailed)?
        {
            let value = match row
                .get_ref(0)
                .map_err(|_| TaggedValueRedactionError::ReadFailed)?
            {
                ValueRef::Text(value) => value,
                ValueRef::Null | ValueRef::Integer(_) | ValueRef::Real(_) | ValueRef::Blob(_) => {
                    return Err(TaggedValueRedactionError::InvalidValue)
                }
            };
            if value.is_empty()
                || value.contains(&0)
                || value.len() > storage::MAX_NODE_BODY_BYTES
                || std::str::from_utf8(value).is_err()
            {
                return Err(TaggedValueRedactionError::InvalidValue);
            }
            if unique.insert(value.to_vec()) {
                if unique.len() > MAX_TAGGED_VALUES {
                    return Err(TaggedValueRedactionError::SourceLimit);
                }
                total_bytes = total_bytes
                    .checked_add(value.len())
                    .ok_or(TaggedValueRedactionError::SourceLimit)?;
                if total_bytes > MAX_TAGGED_TOTAL_BYTES {
                    return Err(TaggedValueRedactionError::SourceLimit);
                }
            }
        }

        let mut values = unique
            .into_iter()
            .map(Vec::into_boxed_slice)
            .collect::<Vec<_>>();
        values.sort_by(|left, right| {
            right
                .len()
                .cmp(&left.len())
                .then_with(|| left.as_ref().cmp(right.as_ref()))
        });
        let mut by_first_byte = vec![Vec::new(); 256];
        for (index, value) in values.iter().enumerate() {
            by_first_byte[usize::from(value[0])].push(index);
        }
        let mut json_escaped = BTreeSet::new();
        let mut json_escaped_total_bytes = 0_usize;
        for value in &values {
            let value =
                std::str::from_utf8(value).map_err(|_| TaggedValueRedactionError::InvalidValue)?;
            let encoded = serde_json::to_string(value)
                .map_err(|_| TaggedValueRedactionError::InvalidValue)?;
            let encoded = encoded
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .ok_or(TaggedValueRedactionError::InvalidValue)?;
            if encoded.as_bytes() != value.as_bytes()
                && json_escaped.insert(encoded.as_bytes().to_vec())
            {
                json_escaped_total_bytes = json_escaped_total_bytes
                    .checked_add(encoded.len())
                    .ok_or(TaggedValueRedactionError::SourceLimit)?;
                if json_escaped_total_bytes > MAX_TAGGED_TOTAL_BYTES {
                    return Err(TaggedValueRedactionError::SourceLimit);
                }
            }
        }
        let mut json_escaped_values = json_escaped
            .into_iter()
            .map(Vec::into_boxed_slice)
            .collect::<Vec<_>>();
        json_escaped_values.sort_by(|left, right| {
            right
                .len()
                .cmp(&left.len())
                .then_with(|| left.as_ref().cmp(right.as_ref()))
        });
        let mut json_escaped_by_first_byte = vec![Vec::new(); 256];
        for (index, value) in json_escaped_values.iter().enumerate() {
            json_escaped_by_first_byte[usize::from(value[0])].push(index);
        }
        Ok(Self {
            values,
            by_first_byte,
            json_escaped_values,
            json_escaped_by_first_byte,
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    #[must_use]
    pub fn contains_exact_value(&self, value: &[u8]) -> bool {
        if self.values.is_empty() || value.is_empty() {
            return false;
        }
        self.find_match(value, 0).is_some()
            || (1..value.len()).any(|offset| self.find_match(value, offset).is_some())
    }

    pub fn redact_str(&self, value: &str) -> Result<String, TaggedValueRedactionError> {
        let redacted = self.redact_bytes(value.as_bytes())?;
        String::from_utf8(redacted).map_err(|_| TaggedValueRedactionError::InvalidValue)
    }

    pub fn redact_str_bounded(
        &self,
        value: &str,
        maximum_bytes: usize,
    ) -> Result<String, TaggedValueRedactionError> {
        let redacted = self.redact_bytes_bounded(value.as_bytes(), maximum_bytes)?;
        String::from_utf8(redacted).map_err(|_| TaggedValueRedactionError::InvalidValue)
    }

    pub fn redact_bytes(&self, value: &[u8]) -> Result<Vec<u8>, TaggedValueRedactionError> {
        let maximum_bytes = value
            .len()
            .checked_add(MAX_REDACTION_EXPANSION_BYTES)
            .ok_or(TaggedValueRedactionError::OutputLimit)?;
        self.redact_bytes_bounded(value, maximum_bytes)
    }

    /// Redacts raw values and their canonical JSON-string representations.
    ///
    /// This is for export rows that can contain serialized JSON copies, such
    /// as teach proposal bodies. Both pattern sets compete at each original
    /// input offset, so leftmost-longest and marker idempotence still hold.
    pub fn redact_bytes_with_json_copies(
        &self,
        value: &[u8],
    ) -> Result<Vec<u8>, TaggedValueRedactionError> {
        let maximum_bytes = value
            .len()
            .checked_add(MAX_REDACTION_EXPANSION_BYTES)
            .ok_or(TaggedValueRedactionError::OutputLimit)?;
        self.redact_bytes_bounded_internal(value, maximum_bytes, true)
    }

    /// Redacts the original bytes in one left-to-right pass.
    ///
    /// Existing markers are copied atomically for idempotence. At each input
    /// offset, the first candidate is the longest deterministic match.
    pub fn redact_bytes_bounded(
        &self,
        value: &[u8],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, TaggedValueRedactionError> {
        self.redact_bytes_bounded_internal(value, maximum_bytes, false)
    }

    fn redact_bytes_bounded_internal(
        &self,
        value: &[u8],
        maximum_bytes: usize,
        include_json_copies: bool,
    ) -> Result<Vec<u8>, TaggedValueRedactionError> {
        if value.len() > maximum_bytes {
            return Err(TaggedValueRedactionError::OutputLimit);
        }
        if self.values.is_empty() && (!include_json_copies || self.json_escaped_values.is_empty()) {
            return Ok(value.to_vec());
        }

        let marker = TEST_SECRET_REDACTION_MARKER.as_bytes();
        let mut output = Vec::with_capacity(value.len().min(maximum_bytes));
        let mut offset = 0_usize;
        while offset < value.len() {
            if value[offset..].starts_with(marker) {
                extend_bounded(&mut output, marker, maximum_bytes)?;
                offset += marker.len();
                continue;
            }
            let matched = if include_json_copies {
                self.find_match_with_json_copies(value, offset)
            } else {
                self.find_match(value, offset)
            };
            if let Some(secret) = matched {
                extend_bounded(&mut output, marker, maximum_bytes)?;
                offset += secret.len();
            } else {
                extend_bounded(&mut output, &value[offset..offset + 1], maximum_bytes)?;
                offset += 1;
            }
        }
        Ok(output)
    }

    fn find_match_with_json_copies<'a>(&'a self, value: &[u8], offset: usize) -> Option<&'a [u8]> {
        let raw = self.find_match(value, offset);
        let first = *value.get(offset)?;
        let escaped = self.json_escaped_by_first_byte[usize::from(first)]
            .iter()
            .map(|index| self.json_escaped_values[*index].as_ref())
            .find(|candidate| value[offset..].starts_with(candidate));
        match (raw, escaped) {
            (Some(raw), Some(escaped)) => Some(
                if raw.len() > escaped.len() || (raw.len() == escaped.len() && raw <= escaped) {
                    raw
                } else {
                    escaped
                },
            ),
            (Some(raw), None) => Some(raw),
            (None, Some(escaped)) => Some(escaped),
            (None, None) => None,
        }
    }

    fn find_match<'a>(&'a self, value: &[u8], offset: usize) -> Option<&'a [u8]> {
        let first = *value.get(offset)?;
        self.by_first_byte[usize::from(first)]
            .iter()
            .map(|index| self.values[*index].as_ref())
            .find(|candidate| value[offset..].starts_with(candidate))
    }
}

fn extend_bounded(
    output: &mut Vec<u8>,
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<(), TaggedValueRedactionError> {
    let next_len = output
        .len()
        .checked_add(bytes.len())
        .ok_or(TaggedValueRedactionError::OutputLimit)?;
    if next_len > maximum_bytes {
        return Err(TaggedValueRedactionError::OutputLimit);
    }
    output.extend_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use rusqlite::{params, Connection};

    fn loaded(values: &[&str]) -> TaggedValueRedactor {
        let mut connection = Connection::open_in_memory().expect("test DB should open");
        schema::apply_migrations(&mut connection).expect("test schema should apply");
        for value in values {
            connection
                .execute(
                    "INSERT INTO nodes (
                        node_type, status, title, body
                     ) VALUES ('raw_note', 'active', 'Authorized test credential', ?1)",
                    [value],
                )
                .expect("tagged node should insert");
            let node_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO tags (node_id, tag) VALUES (?1, ?2)",
                    params![node_id, TEST_SECRET_TAG],
                )
                .expect("test-secret tag should insert");
        }
        TaggedValueRedactor::load(&connection).expect("redactor should load")
    }

    #[test]
    fn tagged_redactor_is_leftmost_longest_deterministic_and_idempotent() {
        let redactor = loaded(&["alpha", "alpha-long", "TEST", "SECRET", "alpha"]);
        let input = "alpha-long alpha TEST SECRET alpha-long";
        let expected = [
            TEST_SECRET_REDACTION_MARKER,
            TEST_SECRET_REDACTION_MARKER,
            TEST_SECRET_REDACTION_MARKER,
            TEST_SECRET_REDACTION_MARKER,
            TEST_SECRET_REDACTION_MARKER,
        ]
        .join(" ");

        let once = redactor.redact_str(input).expect("input should redact");
        let twice = redactor.redact_str(&once).expect("marker should be stable");

        assert_eq!(once, expected);
        assert_eq!(twice, expected);
    }

    #[test]
    fn tagged_redactor_can_scrub_canonical_json_string_copies_in_one_pass() {
        let secret = "quote\" slash\\ line\nnext";
        let redactor = loaded(&[secret]);
        let encoded = serde_json::to_string(secret).expect("fake secret should encode");
        let encoded = encoded
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .expect("encoded JSON string should have quotes");
        let input = format!("raw={secret}; json={encoded}");

        let output = redactor
            .redact_bytes_with_json_copies(input.as_bytes())
            .expect("raw and JSON copies should redact");
        let output = String::from_utf8(output).expect("redacted output should stay UTF-8");

        assert_eq!(
            output,
            format!(
                "raw={}; json={}",
                TEST_SECRET_REDACTION_MARKER, TEST_SECRET_REDACTION_MARKER
            )
        );
        assert_eq!(
            redactor
                .redact_bytes_with_json_copies(output.as_bytes())
                .expect("marker should remain idempotent"),
            output.as_bytes()
        );
    }

    #[test]
    fn tagged_redactor_requires_exact_binary_tag_and_valid_body() {
        let mut connection = Connection::open_in_memory().expect("test DB should open");
        schema::apply_migrations(&mut connection).expect("test schema should apply");
        connection
            .execute(
                "INSERT INTO nodes (node_type, status, title, body)
                 VALUES ('raw_note', 'active', 'case mismatch', 'not-loaded')",
                [],
            )
            .expect("case fixture should insert");
        connection
            .execute(
                "INSERT INTO tags (node_id, tag) VALUES (?1, ?2)",
                params![connection.last_insert_rowid(), "Sensitivity:test_secret"],
            )
            .expect("case tag should insert");
        assert!(TaggedValueRedactor::load(&connection)
            .expect("case mismatch should be ignored")
            .is_empty());

        connection
            .execute(
                "INSERT INTO nodes (node_type, status, title, body)
                 VALUES ('raw_note', 'active', 'missing body', NULL)",
                [],
            )
            .expect("missing-body fixture should insert");
        connection
            .execute(
                "INSERT INTO tags (node_id, tag) VALUES (?1, ?2)",
                params![connection.last_insert_rowid(), TEST_SECRET_TAG],
            )
            .expect("exact tag should insert");
        assert!(matches!(
            TaggedValueRedactor::load(&connection),
            Err(TaggedValueRedactionError::InvalidValue)
        ));
    }

    #[test]
    fn tagged_redactor_bounds_output_expansion_without_partial_marker() {
        let redactor = loaded(&["x"]);
        let error = redactor
            .redact_bytes_bounded(b"xxxx", TEST_SECRET_REDACTION_MARKER.len() * 3)
            .expect_err("four markers must exceed a three-marker bound");
        assert_eq!(error, TaggedValueRedactionError::OutputLimit);
    }

    #[test]
    fn stage_010_tagged_source_count_and_body_bounds_fail_closed() {
        let mut count_connection =
            Connection::open_in_memory().expect("count-bound DB should open");
        schema::apply_migrations(&mut count_connection).expect("test schema should apply");
        let transaction = count_connection
            .transaction()
            .expect("count-bound transaction should start");
        for index in 0..=MAX_TAGGED_VALUES {
            transaction
                .execute(
                    "INSERT INTO nodes (node_type, status, title, body)
                     VALUES ('raw_note', 'active', 'count bound', ?1)",
                    [format!("stage-010-secret-{index:04}")],
                )
                .expect("count-bound node should insert");
            transaction
                .execute(
                    "INSERT INTO tags (node_id, tag) VALUES (?1, ?2)",
                    params![transaction.last_insert_rowid(), TEST_SECRET_TAG],
                )
                .expect("count-bound tag should insert");
        }
        transaction
            .commit()
            .expect("count-bound transaction should commit");
        assert!(matches!(
            TaggedValueRedactor::load(&count_connection),
            Err(TaggedValueRedactionError::SourceLimit)
        ));

        let mut body_connection = Connection::open_in_memory().expect("body-bound DB should open");
        schema::apply_migrations(&mut body_connection).expect("test schema should apply");
        body_connection
            .execute(
                "INSERT INTO nodes (node_type, status, title, body)
                 VALUES ('raw_note', 'active', 'body bound', ?1)",
                ["x".repeat(storage::MAX_NODE_BODY_BYTES + 1)],
            )
            .expect("oversized raw fixture should insert");
        body_connection
            .execute(
                "INSERT INTO tags (node_id, tag) VALUES (?1, ?2)",
                params![body_connection.last_insert_rowid(), TEST_SECRET_TAG],
            )
            .expect("body-bound tag should insert");
        assert!(matches!(
            TaggedValueRedactor::load(&body_connection),
            Err(TaggedValueRedactionError::InvalidValue)
        ));
    }

    #[test]
    fn stage_010_tagged_source_total_bytes_bound_fails_closed() {
        let mut connection = Connection::open_in_memory().expect("total-bound DB should open");
        schema::apply_migrations(&mut connection).expect("test schema should apply");
        let transaction = connection
            .transaction()
            .expect("total-bound transaction should start");
        for index in 0..=MAX_TAGGED_TOTAL_BYTES / storage::MAX_NODE_BODY_BYTES {
            let prefix = format!("{index:02}");
            let value = format!(
                "{prefix}{}",
                "x".repeat(storage::MAX_NODE_BODY_BYTES - prefix.len())
            );
            transaction
                .execute(
                    "INSERT INTO nodes (node_type, status, title, body)
                     VALUES ('raw_note', 'active', 'total bound', ?1)",
                    [value],
                )
                .expect("total-bound node should insert");
            transaction
                .execute(
                    "INSERT INTO tags (node_id, tag) VALUES (?1, ?2)",
                    params![transaction.last_insert_rowid(), TEST_SECRET_TAG],
                )
                .expect("total-bound tag should insert");
        }
        transaction
            .commit()
            .expect("total-bound transaction should commit");

        assert!(matches!(
            TaggedValueRedactor::load(&connection),
            Err(TaggedValueRedactionError::SourceLimit)
        ));
    }
}
