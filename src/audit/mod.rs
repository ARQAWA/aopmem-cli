use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

pub const NODE_CREATED_EVENT: &str = "node.created";
pub const NODE_UPDATED_EVENT: &str = "node.updated";
pub const LINK_CREATED_EVENT: &str = "link.created";

const NODE_SUBJECT: &str = "node";
const LINK_SUBJECT: &str = "link";
const SNAPSHOT_FILE_NAME: &str = "memory.sql";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    pub id: i64,
    pub event_type: String,
    pub timestamp: String,
    pub source: String,
    pub subject_kind: String,
    pub subject_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SqlSnapshotReport {
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum SnapshotError {
    Db(rusqlite::Error),
    Io(std::io::Error),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Db(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventValidationError {
    MissingType,
    MissingSource,
    InvalidSubjectKind(String),
    InvalidSubjectId(i64),
}

impl fmt::Display for EventValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingType => write!(formatter, "missing required field: type"),
            Self::MissingSource => write!(formatter, "missing required field: source"),
            Self::InvalidSubjectKind(kind) => write!(formatter, "invalid subject kind: {kind}"),
            Self::InvalidSubjectId(id) => write!(formatter, "invalid subject id: {id}"),
        }
    }
}

impl std::error::Error for EventValidationError {}

#[derive(Debug)]
pub enum AuditError {
    Validation(EventValidationError),
    Db(rusqlite::Error),
}

impl fmt::Display for AuditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "{error}"),
            Self::Db(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AuditError {}

impl From<EventValidationError> for AuditError {
    fn from(error: EventValidationError) -> Self {
        Self::Validation(error)
    }
}

impl From<rusqlite::Error> for AuditError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

impl From<rusqlite::Error> for SnapshotError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

impl From<std::io::Error> for SnapshotError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn record_node_created(
    connection: &Connection,
    node_id: i64,
    source: &str,
) -> Result<Event, AuditError> {
    record_event(
        connection,
        NODE_CREATED_EVENT,
        source,
        NODE_SUBJECT,
        node_id,
    )
}

pub fn record_node_updated(
    connection: &Connection,
    node_id: i64,
    source: &str,
) -> Result<Event, AuditError> {
    record_event(
        connection,
        NODE_UPDATED_EVENT,
        source,
        NODE_SUBJECT,
        node_id,
    )
}

pub fn record_link_created(
    connection: &Connection,
    link_id: i64,
    source: &str,
) -> Result<Event, AuditError> {
    record_event(
        connection,
        LINK_CREATED_EVENT,
        source,
        LINK_SUBJECT,
        link_id,
    )
}

pub fn list_events(connection: &Connection) -> rusqlite::Result<Vec<Event>> {
    let mut statement = connection.prepare(
        "
        SELECT id, type, timestamp, source, subject_kind, subject_id
        FROM events
        ORDER BY id ASC;
        ",
    )?;

    let events = statement.query_map([], row_to_event)?.collect();
    events
}

pub fn write_sql_snapshot(
    audit_git_dir: &Path,
    connection: &Connection,
) -> Result<SqlSnapshotReport, SnapshotError> {
    fs::create_dir_all(audit_git_dir)?;

    let dump = build_sql_dump(connection)?;
    let path = audit_git_dir.join(SNAPSHOT_FILE_NAME);
    fs::write(&path, dump)?;

    Ok(SqlSnapshotReport { path })
}

fn record_event(
    connection: &Connection,
    event_type: &str,
    source: &str,
    subject_kind: &str,
    subject_id: i64,
) -> Result<Event, AuditError> {
    validate_event(event_type, source, subject_kind, subject_id)?;

    connection.execute(
        "
        INSERT INTO events (type, source, subject_kind, subject_id)
        VALUES (?1, ?2, ?3, ?4);
        ",
        params![event_type, source, subject_kind, subject_id],
    )?;

    let id = connection.last_insert_rowid();
    get_event(connection, id)?.ok_or(AuditError::Db(rusqlite::Error::QueryReturnedNoRows))
}

fn validate_event(
    event_type: &str,
    source: &str,
    subject_kind: &str,
    subject_id: i64,
) -> Result<(), EventValidationError> {
    if event_type.trim().is_empty() {
        return Err(EventValidationError::MissingType);
    }
    if source.trim().is_empty() {
        return Err(EventValidationError::MissingSource);
    }
    if ![NODE_SUBJECT, LINK_SUBJECT].contains(&subject_kind) {
        return Err(EventValidationError::InvalidSubjectKind(
            subject_kind.to_string(),
        ));
    }
    if subject_id <= 0 {
        return Err(EventValidationError::InvalidSubjectId(subject_id));
    }

    Ok(())
}

fn get_event(connection: &Connection, id: i64) -> rusqlite::Result<Option<Event>> {
    connection
        .query_row(
            "
            SELECT id, type, timestamp, source, subject_kind, subject_id
            FROM events
            WHERE id = ?1;
            ",
            [id],
            row_to_event,
        )
        .optional()
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    Ok(Event {
        id: row.get(0)?,
        event_type: row.get(1)?,
        timestamp: row.get(2)?,
        source: row.get(3)?,
        subject_kind: row.get(4)?,
        subject_id: row.get(5)?,
    })
}

fn build_sql_dump(connection: &Connection) -> rusqlite::Result<String> {
    let mut dump = String::new();
    dump.push_str("BEGIN TRANSACTION;\n");

    let mut schema_statement = connection.prepare(
        "
        SELECT type, name, sql
        FROM sqlite_master
        WHERE sql IS NOT NULL
          AND type IN ('table', 'index', 'trigger', 'view')
          AND name NOT LIKE 'sqlite_%'
        ORDER BY
            CASE type
                WHEN 'table' THEN 0
                WHEN 'index' THEN 1
                WHEN 'trigger' THEN 2
                WHEN 'view' THEN 3
                ELSE 4
            END,
            name ASC;
        ",
    )?;
    let schema_rows = schema_statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut table_names = Vec::new();
    for row in schema_rows {
        let (object_type, name, sql) = row?;
        dump.push_str(&sql);
        dump.push_str(";\n");

        if object_type == "table" {
            table_names.push(name);
        }
    }

    for table_name in table_names {
        append_table_rows(&mut dump, connection, &table_name)?;
    }

    dump.push_str("COMMIT;\n");
    Ok(dump)
}

fn append_table_rows(
    dump: &mut String,
    connection: &Connection,
    table_name: &str,
) -> rusqlite::Result<()> {
    let preview_query = format!("SELECT * FROM {} LIMIT 0;", quote_identifier(table_name));
    let preview = connection.prepare(&preview_query)?;
    let column_names = preview
        .column_names()
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    drop(preview);

    let order_clause = if column_names.is_empty() {
        String::new()
    } else {
        format!(
            " ORDER BY {}",
            column_names
                .iter()
                .map(|name| quote_identifier(name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let query = format!(
        "SELECT * FROM {}{};",
        quote_identifier(table_name),
        order_clause
    );
    let mut statement = connection.prepare(&query)?;
    let mut rows = statement.query([])?;

    while let Some(row) = rows.next()? {
        write!(dump, "INSERT INTO {} (", quote_identifier(table_name))
            .expect("string writes should not fail");

        for (index, column_name) in column_names.iter().enumerate() {
            if index > 0 {
                dump.push_str(", ");
            }
            dump.push_str(&quote_identifier(column_name));
        }

        dump.push_str(") VALUES (");

        for index in 0..column_names.len() {
            if index > 0 {
                dump.push_str(", ");
            }
            dump.push_str(&render_sql_value(row.get_ref(index)?));
        }

        dump.push_str(");\n");
    }

    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn render_sql_value(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => "NULL".to_string(),
        ValueRef::Integer(number) => number.to_string(),
        ValueRef::Real(number) => number.to_string(),
        ValueRef::Text(text) => quote_sql_text(&String::from_utf8_lossy(text)),
        ValueRef::Blob(bytes) => {
            let mut encoded = String::with_capacity(bytes.len() * 2 + 3);
            encoded.push_str("X'");
            for byte in bytes {
                write!(&mut encoded, "{byte:02X}").expect("string writes should not fail");
            }
            encoded.push('\'');
            encoded
        }
    }
}

fn quote_sql_text(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn migrated_connection() -> Connection {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for audit test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        connection
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("aopmem-stage-037-audit-{name}-{nanos}"))
    }

    #[test]
    fn records_node_created_event_with_timestamp_and_source() {
        let connection = migrated_connection();

        let event = record_node_created(&connection, 7, "source=user_instruction")
            .expect("node created event should be recorded");
        let events = list_events(&connection).expect("events should list");

        assert_eq!(event.event_type, NODE_CREATED_EVENT);
        assert_eq!(event.source, "source=user_instruction");
        assert_eq!(event.subject_kind, NODE_SUBJECT);
        assert_eq!(event.subject_id, 7);
        assert!(!event.timestamp.trim().is_empty());
        assert_eq!(events, vec![event]);
    }

    #[test]
    fn records_link_created_event_with_timestamp_and_source() {
        let connection = migrated_connection();

        let event = record_link_created(&connection, 11, "source=cli")
            .expect("link created event should be recorded");

        assert_eq!(event.event_type, LINK_CREATED_EVENT);
        assert_eq!(event.source, "source=cli");
        assert_eq!(event.subject_kind, LINK_SUBJECT);
        assert_eq!(event.subject_id, 11);
        assert!(!event.timestamp.trim().is_empty());
    }

    #[test]
    fn rejects_event_without_source_or_valid_subject_id() {
        let connection = migrated_connection();

        assert!(matches!(
            record_node_created(&connection, 1, " "),
            Err(AuditError::Validation(EventValidationError::MissingSource))
        ));
        assert!(matches!(
            record_link_created(&connection, 0, "source=cli"),
            Err(AuditError::Validation(
                EventValidationError::InvalidSubjectId(0)
            ))
        ));
    }

    #[test]
    fn builds_sql_dump_from_migrated_db_with_sample_rows() {
        let connection = migrated_connection();
        connection
            .execute(
                "
                INSERT INTO nodes (
                    node_type,
                    status,
                    title,
                    summary,
                    body,
                    source_ref,
                    confidence,
                    trust_level
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);
                ",
                params![
                    "fact",
                    "active",
                    "O'Hara",
                    "short summary",
                    "body text",
                    "source://note",
                    0.9_f64,
                    "high"
                ],
            )
            .expect("node should insert");
        record_node_created(&connection, 1, "source=cli").expect("event should record");

        let dump = build_sql_dump(&connection).expect("sql dump should build");

        assert!(dump.starts_with("BEGIN TRANSACTION;\n"));
        assert!(dump.contains("CREATE TABLE nodes"));
        assert!(dump.contains("CREATE VIRTUAL TABLE fts_nodes"));
        assert!(dump.contains("INSERT INTO \"nodes\""));
        assert!(dump.contains("'O''Hara'"));
        assert!(dump.contains("INSERT INTO \"events\""));
        assert!(dump.ends_with("COMMIT;\n"));
    }

    #[test]
    fn writes_sql_snapshot_as_text_file_under_audit_git_dir() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("audit-git");
        fs::create_dir_all(&audit_git_dir).expect("audit dir should be created");

        let report =
            write_sql_snapshot(&audit_git_dir, &connection).expect("snapshot should be written");
        let snapshot_text =
            fs::read_to_string(&report.path).expect("snapshot file should be readable as text");

        assert_eq!(report.path, audit_git_dir.join(SNAPSHOT_FILE_NAME));
        assert!(snapshot_text.contains("BEGIN TRANSACTION;"));
        assert!(snapshot_text.contains("CREATE TABLE schema_migrations"));

        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should be removed");
    }
}
