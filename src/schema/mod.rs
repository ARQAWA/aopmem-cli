use std::collections::BTreeMap;

use rusqlite::{Connection, TransactionBehavior};

pub const MIGRATION_001_INIT: &str = "001_init";
pub const MIGRATION_002_NODES_SUMMARY_INDEX: &str = "002_nodes_summary_index";
pub const MIGRATION_003_TASK_RECALL_EXACT_INDEXES: &str = "003_task_recall_exact_indexes";
pub const MIGRATION_004_TASK_PROTOCOL_AND_TOOL_ALIASES: &str = "004_task_protocol_and_tool_aliases";

struct Migration {
    version: &'static str,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "001",
        name: MIGRATION_001_INIT,
        sql: "
        CREATE TABLE IF NOT EXISTS nodes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            node_type TEXT NOT NULL,
            status TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT,
            body TEXT,
            source_ref TEXT,
            confidence REAL,
            trust_level TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_nodes_type ON nodes(node_type);
        CREATE INDEX IF NOT EXISTS idx_nodes_status ON nodes(status);

        CREATE TABLE IF NOT EXISTS links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_node_id INTEGER NOT NULL,
            target_node_id INTEGER NOT NULL,
            link_type TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (source_node_id) REFERENCES nodes(id) ON DELETE RESTRICT,
            FOREIGN KEY (target_node_id) REFERENCES nodes(id) ON DELETE RESTRICT
        );
        CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_node_id);
        CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_node_id);
        CREATE INDEX IF NOT EXISTS idx_links_type ON links(link_type);

        CREATE TABLE IF NOT EXISTS aliases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            node_id INTEGER NOT NULL,
            alias TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE RESTRICT,
            UNIQUE (node_id, alias)
        );
        CREATE INDEX IF NOT EXISTS idx_aliases_node ON aliases(node_id);
        CREATE INDEX IF NOT EXISTS idx_aliases_alias ON aliases(alias);

        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            node_id INTEGER NOT NULL,
            tag TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE RESTRICT,
            UNIQUE (node_id, tag)
        );
        CREATE INDEX IF NOT EXISTS idx_tags_node ON tags(node_id);
        CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);

        CREATE TABLE IF NOT EXISTS sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            node_id INTEGER NOT NULL,
            source_ref TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE RESTRICT,
            UNIQUE (node_id, source_ref)
        );
        CREATE INDEX IF NOT EXISTS idx_sources_node ON sources(node_id);
        CREATE INDEX IF NOT EXISTS idx_sources_ref ON sources(source_ref);

        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL,
            timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            source TEXT NOT NULL,
            subject_kind TEXT NOT NULL,
            subject_id INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_events_type ON events(type);
        CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
        CREATE INDEX IF NOT EXISTS idx_events_subject ON events(subject_kind, subject_id);

        CREATE TABLE IF NOT EXISTS registries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            registry_type TEXT NOT NULL,
            name TEXT NOT NULL,
            status TEXT NOT NULL,
            notes TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (registry_type, name)
        );
        CREATE INDEX IF NOT EXISTS idx_registries_type ON registries(registry_type);
        CREATE INDEX IF NOT EXISTS idx_registries_status ON registries(status);

        CREATE TABLE IF NOT EXISTS tool_contracts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tool_id TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            status TEXT NOT NULL,
            owner_workflow TEXT,
            side_effects TEXT NOT NULL,
            approval_requirement TEXT NOT NULL,
            contract_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_tool_contracts_status ON tool_contracts(status);

        CREATE TABLE IF NOT EXISTS mcp_profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            status TEXT NOT NULL,
            read_operations TEXT NOT NULL,
            write_operations TEXT NOT NULL,
            side_effects TEXT NOT NULL,
            approval_requirement TEXT NOT NULL,
            credentials_source TEXT,
            notes TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_mcp_profiles_kind ON mcp_profiles(kind);
        CREATE INDEX IF NOT EXISTS idx_mcp_profiles_status ON mcp_profiles(status);

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_nodes USING fts5(
            title,
            summary,
            body,
            aliases
        );
        ",
    },
    Migration {
        version: "002",
        name: MIGRATION_002_NODES_SUMMARY_INDEX,
        sql: "
            CREATE INDEX IF NOT EXISTS idx_nodes_summary ON nodes(summary);
        ",
    },
    Migration {
        version: "003",
        name: MIGRATION_003_TASK_RECALL_EXACT_INDEXES,
        sql: "
            CREATE INDEX IF NOT EXISTS idx_nodes_title_nocase
                ON nodes(title COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS idx_aliases_alias_nocase
                ON aliases(alias COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS idx_tags_tag_nocase
                ON tags(tag COLLATE NOCASE);
        ",
    },
    Migration {
        version: "004",
        name: MIGRATION_004_TASK_PROTOCOL_AND_TOOL_ALIASES,
        sql: "
            CREATE TABLE tool_aliases (
                alias TEXT PRIMARY KEY
                    CHECK (
                        typeof(alias) = 'text'
                        AND length(CAST(alias AS BLOB)) BETWEEN 1 AND 128
                        AND trim(alias) <> ''
                        AND instr(alias, char(0)) = 0
                    ),
                canonical_tool_id TEXT NOT NULL
                    CHECK (
                        typeof(canonical_tool_id) = 'text'
                        AND length(CAST(canonical_tool_id AS BLOB)) BETWEEN 1 AND 128
                        AND trim(canonical_tool_id) <> ''
                        AND instr(canonical_tool_id, char(0)) = 0
                    ),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                source TEXT NOT NULL
                    CHECK (
                        typeof(source) = 'text'
                        AND length(CAST(source AS BLOB)) BETWEEN 1 AND 128
                        AND trim(source) <> ''
                        AND instr(source, char(0)) = 0
                    ),
                status TEXT NOT NULL DEFAULT 'active'
                    CHECK (status = 'active'),
                FOREIGN KEY (canonical_tool_id)
                    REFERENCES tool_contracts(tool_id) ON DELETE RESTRICT
            );
            CREATE INDEX idx_tool_aliases_status_alias
                ON tool_aliases(status, alias);
            CREATE INDEX idx_tool_aliases_canonical_status_alias
                ON tool_aliases(canonical_tool_id, status, alias);

            CREATE TRIGGER tool_aliases_validate_insert
            BEFORE INSERT ON tool_aliases
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM tool_contracts
                    WHERE tool_id = NEW.canonical_tool_id
                      AND status = 'active'
                ) THEN RAISE(ABORT, 'tool alias target must be an active canonical tool') END;
                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM tool_aliases
                    WHERE alias = NEW.canonical_tool_id
                      AND status = 'active'
                ) THEN RAISE(ABORT, 'tool alias target cannot be another alias') END;
                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM tool_contracts
                    WHERE tool_id = NEW.alias
                      AND status <> 'superseded'
                ) THEN RAISE(ABORT, 'tool alias cannot shadow a non-superseded tool') END;
            END;

            CREATE TRIGGER tool_aliases_validate_update
            BEFORE UPDATE ON tool_aliases
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM tool_contracts
                    WHERE tool_id = NEW.canonical_tool_id
                      AND status = 'active'
                ) THEN RAISE(ABORT, 'tool alias target must be an active canonical tool') END;
                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM tool_aliases
                    WHERE alias = NEW.canonical_tool_id
                      AND status = 'active'
                ) THEN RAISE(ABORT, 'tool alias target cannot be another alias') END;
                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM tool_contracts
                    WHERE tool_id = NEW.alias
                      AND status <> 'superseded'
                ) THEN RAISE(ABORT, 'tool alias cannot shadow a non-superseded tool') END;
            END;

            CREATE TRIGGER tool_contracts_reject_alias_shadow_insert
            BEFORE INSERT ON tool_contracts
            WHEN NEW.status <> 'superseded'
                 AND EXISTS (
                    SELECT 1 FROM tool_aliases WHERE alias = NEW.tool_id
                 )
            BEGIN
                SELECT RAISE(ABORT, 'non-superseded tool cannot shadow an alias');
            END;

            CREATE TRIGGER tool_contracts_reject_alias_shadow_update
            BEFORE UPDATE OF tool_id, status ON tool_contracts
            WHEN NEW.status <> 'superseded'
                 AND EXISTS (
                    SELECT 1 FROM tool_aliases WHERE alias = NEW.tool_id
                 )
            BEGIN
                SELECT RAISE(ABORT, 'non-superseded tool cannot shadow an alias');
            END;

            CREATE TRIGGER tool_contracts_preserve_alias_target
            BEFORE UPDATE OF tool_id, status ON tool_contracts
            WHEN EXISTS (
                     SELECT 1
                     FROM tool_aliases
                     WHERE canonical_tool_id = OLD.tool_id
                       AND status = 'active'
                 )
                 AND (
                     NEW.tool_id <> OLD.tool_id
                     OR NEW.status <> 'active'
                 )
            BEGIN
                SELECT RAISE(ABORT, 'tool alias target must remain active and canonical');
            END;
        ",
    },
];

pub fn apply_migrations(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    apply_pending_migrations_in(&transaction)?;
    transaction.commit()
}

/// Applies pending migrations inside the caller's active transaction.
///
/// The caller owns commit and rollback. This is used by the workspace mutation
/// coordinator so schema changes and the requested write have one atomic fate.
pub fn apply_pending_migrations_in(connection: &Connection) -> rusqlite::Result<()> {
    apply_pending_migrations_from(connection, MIGRATIONS)
}

fn apply_pending_migrations_from(
    connection: &Connection,
    migrations: &[Migration],
) -> rusqlite::Result<()> {
    ensure_schema_migrations_table(connection)?;

    let applied_migrations = applied_migrations(connection)?;
    validate_applied_migrations(&applied_migrations, migrations)?;
    let pending_migrations = migrations
        .iter()
        .filter(|migration| !applied_migrations.contains_key(migration.version))
        .collect::<Vec<_>>();

    for migration in pending_migrations {
        connection.execute_batch(migration.sql)?;
        connection.execute(
            "
            INSERT INTO schema_migrations (version, name)
            VALUES (?1, ?2);
            ",
            (migration.version, migration.name),
        )?;
    }
    Ok(())
}

#[cfg(test)]
fn apply_migrations_from(
    connection: &mut Connection,
    migrations: &[Migration],
) -> rusqlite::Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    apply_pending_migrations_from(&transaction, migrations)?;
    transaction.commit()
}

fn applied_migrations(connection: &Connection) -> rusqlite::Result<BTreeMap<String, String>> {
    let mut statement =
        connection.prepare("SELECT version, name FROM schema_migrations ORDER BY version;")?;
    let migrations = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
    Ok(migrations)
}

fn validate_applied_migrations(
    applied: &BTreeMap<String, String>,
    known: &[Migration],
) -> rusqlite::Result<()> {
    let mut known_by_version = BTreeMap::new();
    for migration in known {
        if known_by_version
            .insert(migration.version, migration.name)
            .is_some()
        {
            return Err(migration_integrity_error(format!(
                "duplicate known schema migration version: {}",
                migration.version
            )));
        }
    }

    for (version, actual_name) in applied {
        let Some(expected_name) = known_by_version.get(version.as_str()) else {
            return Err(migration_integrity_error(format!(
                "unknown schema migration version: {version} ({actual_name})"
            )));
        };
        if actual_name != expected_name {
            return Err(migration_integrity_error(format!(
                "schema migration {version} name mismatch: expected {expected_name}, found {actual_name}"
            )));
        }
    }

    Ok(())
}

fn migration_integrity_error(message: String) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message)
}

fn ensure_schema_migrations_table(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        ",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_migrations_creates_schema_migrations_and_001_init_marker() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");

        apply_migrations(&mut connection).expect("migrations should apply");

        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = '001' AND name = '001_init';",
                [],
                |row| row.get(0),
            )
            .expect("schema_migrations should be queryable");

        assert_eq!(count, 1);
    }

    #[test]
    fn apply_migrations_creates_nodes_summary_index() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");

        apply_migrations(&mut connection).expect("migrations should apply");

        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_nodes_summary';",
                [],
                |row| row.get(0),
            )
            .expect("sqlite_master should be queryable");

        assert_eq!(count, 1);
    }

    #[test]
    fn apply_migrations_is_idempotent() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");

        apply_migrations(&mut connection).expect("first migration run should pass");
        apply_migrations(&mut connection).expect("second migration run should pass");

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations;", [], |row| {
                row.get(0)
            })
            .expect("schema_migrations should be queryable");

        assert_eq!(count, 4);
    }

    #[test]
    fn apply_migrations_runs_only_the_missing_version() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");

        apply_migrations(&mut connection).expect("first migration run should pass");
        connection
            .execute("DELETE FROM schema_migrations WHERE version = '002';", [])
            .expect("second migration marker should be removable for test");
        connection
            .execute_batch("DROP INDEX idx_nodes_summary;")
            .expect("second migration index should be removable for test");

        apply_migrations(&mut connection).expect("missing migration should rerun");

        let marker_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = '002';",
                [],
                |row| row.get(0),
            )
            .expect("second migration marker should be restored");
        let index_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_nodes_summary';",
                [],
                |row| row.get(0),
            )
            .expect("second migration index should be restored");

        assert_eq!(marker_count, 1);
        assert_eq!(index_count, 1);
    }

    #[test]
    fn v010_fixture_applies_pending_indexes_and_preserves_existing_rows() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for v0.1 fixture");
        connection
            .execute_batch(
                "
                CREATE TABLE schema_migrations (
                    version TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO schema_migrations (version, name) VALUES ('001', '001_init');
                CREATE TABLE nodes (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    node_type TEXT NOT NULL,
                    status TEXT NOT NULL,
                    title TEXT NOT NULL,
                    summary TEXT,
                    body TEXT,
                    source_ref TEXT,
                    confidence REAL,
                    trust_level TEXT,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE aliases (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    node_id INTEGER NOT NULL,
                    alias TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE tags (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    node_id INTEGER NOT NULL,
                    tag TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE tool_contracts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    tool_id TEXT NOT NULL UNIQUE,
                    name TEXT NOT NULL,
                    status TEXT NOT NULL,
                    owner_workflow TEXT,
                    side_effects TEXT NOT NULL,
                    approval_requirement TEXT NOT NULL,
                    contract_json TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO nodes (node_type, status, title)
                VALUES ('raw_note', 'draft', 'preserve-v010-row');
                ",
            )
            .expect("v0.1 fixture should initialize");

        apply_migrations(&mut connection).expect("v0.1 fixture should upgrade");

        let title: String = connection
            .query_row("SELECT title FROM nodes WHERE id = 1;", [], |row| {
                row.get(0)
            })
            .expect("existing v0.1 row should remain readable");
        let migrations: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations;", [], |row| {
                row.get(0)
            })
            .expect("migration markers should be readable");
        assert_eq!(title, "preserve-v010-row");
        assert_eq!(migrations, 4);
        assert!(object_exists(&connection, "index", "idx_nodes_summary"));
        assert!(object_exists(
            &connection,
            "index",
            "idx_nodes_title_nocase"
        ));
        assert!(object_exists(
            &connection,
            "index",
            "idx_aliases_alias_nocase"
        ));
        assert!(object_exists(&connection, "index", "idx_tags_tag_nocase"));
    }

    #[test]
    fn task_recall_exact_index_migration_creates_all_indexes_and_marker() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for index migration");

        apply_migrations(&mut connection).expect("migrations should apply");

        for index in [
            "idx_nodes_title_nocase",
            "idx_aliases_alias_nocase",
            "idx_tags_tag_nocase",
        ] {
            assert!(
                object_exists(&connection, "index", index),
                "missing {index}"
            );
        }
        let marker: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = '003' AND name = ?1;",
                [MIGRATION_003_TASK_RECALL_EXACT_INDEXES],
                |row| row.get(0),
            )
            .expect("task recall migration marker should be readable");
        assert_eq!(marker, 1);
    }

    #[test]
    fn task_recall_exact_index_migration_rolls_back_with_later_failure() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for rollback fixture");
        apply_migrations_from(&mut connection, &MIGRATIONS[..2])
            .expect("v0.1-compatible base fixture should migrate");
        let mut migrations = MIGRATIONS
            .iter()
            .map(|migration| Migration {
                version: migration.version,
                name: migration.name,
                sql: migration.sql,
            })
            .collect::<Vec<_>>();
        migrations.push(Migration {
            version: "005",
            name: "005_forced_failure",
            sql: "CREATE TABLE invalid_sql (",
        });

        apply_migrations_from(&mut connection, &migrations)
            .expect_err("later migration failure should roll back pending indexes");

        for index in [
            "idx_nodes_title_nocase",
            "idx_aliases_alias_nocase",
            "idx_tags_tag_nocase",
        ] {
            assert!(!object_exists(&connection, "index", index));
        }
        let marker: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = '003';",
                [],
                |row| row.get(0),
            )
            .expect("migration marker should remain readable");
        assert_eq!(marker, 0);
        assert!(connection.is_autocommit());
    }

    #[test]
    fn stage_011_tool_alias_migration_has_required_schema_and_marker() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for alias migration");

        apply_migrations(&mut connection).expect("migrations should apply");

        let columns = connection
            .prepare("PRAGMA table_info('tool_aliases')")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .expect("tool alias columns should be readable");
        assert_eq!(
            columns,
            [
                "alias",
                "canonical_tool_id",
                "created_at",
                "source",
                "status"
            ]
        );
        for index in [
            "idx_tool_aliases_status_alias",
            "idx_tool_aliases_canonical_status_alias",
        ] {
            assert!(
                object_exists(&connection, "index", index),
                "missing {index}"
            );
        }
        for trigger in [
            "tool_aliases_validate_insert",
            "tool_aliases_validate_update",
            "tool_contracts_reject_alias_shadow_insert",
            "tool_contracts_reject_alias_shadow_update",
            "tool_contracts_preserve_alias_target",
        ] {
            assert!(
                object_exists(&connection, "trigger", trigger),
                "missing {trigger}"
            );
        }
        let marker: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations
                 WHERE version = '004' AND name = ?1",
                [MIGRATION_004_TASK_PROTOCOL_AND_TOOL_ALIASES],
                |row| row.get(0),
            )
            .expect("tool alias migration marker should be readable");
        assert_eq!(marker, 1);
    }

    #[test]
    fn stage_011_tool_alias_migration_upgrades_001_and_003_sources() {
        for completed in [1_usize, 3_usize] {
            let mut connection = Connection::open_in_memory()
                .expect("in-memory DB should open for migration source");
            apply_migrations_from(&mut connection, &MIGRATIONS[..completed])
                .expect("source migrations should apply");
            connection
                .execute(
                    "INSERT INTO tool_contracts (
                        tool_id, name, status, side_effects,
                        approval_requirement, contract_json
                     ) VALUES ('canonical', 'Canonical', 'active', 'none', 'none', '{}')",
                    [],
                )
                .expect("source tool should insert");

            apply_migrations(&mut connection).expect("source should upgrade to 004");
            connection
                .execute(
                    "INSERT INTO tool_aliases (
                        alias, canonical_tool_id, source, status
                     ) VALUES ('old-id', 'canonical', 'migration-test', 'active')",
                    [],
                )
                .expect("alias should insert after migration");

            let rows: i64 = connection
                .query_row("SELECT COUNT(*) FROM tool_aliases", [], |row| row.get(0))
                .expect("tool aliases should be readable");
            assert_eq!(rows, 1);
        }
    }

    #[test]
    fn stage_011_tool_alias_migration_rolls_back_with_later_failure() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for rollback fixture");
        apply_migrations_from(&mut connection, &MIGRATIONS[..3])
            .expect("003 source fixture should migrate");
        let mut migrations = MIGRATIONS
            .iter()
            .map(|migration| Migration {
                version: migration.version,
                name: migration.name,
                sql: migration.sql,
            })
            .collect::<Vec<_>>();
        migrations.push(Migration {
            version: "005",
            name: "005_forced_failure_after_tool_aliases",
            sql: "CREATE TABLE invalid_sql (",
        });

        apply_migrations_from(&mut connection, &migrations)
            .expect_err("later failure should roll back tool alias migration");

        assert!(!object_exists(&connection, "table", "tool_aliases"));
        let marker: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = '004'",
                [],
                |row| row.get(0),
            )
            .expect("migration marker should remain readable");
        assert_eq!(marker, 0);
        assert!(connection.is_autocommit());
    }

    #[test]
    fn migration_rejects_unknown_version_without_applying_pending_changes() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");
        seed_partial_schema(&connection, "001", "001_init");
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name) VALUES ('999', 'future');",
                [],
            )
            .expect("future marker should seed");

        let error = apply_migrations(&mut connection)
            .expect_err("unknown migration version must be rejected");

        assert!(error
            .to_string()
            .contains("unknown schema migration version: 999"));
        assert!(!object_exists(&connection, "index", "idx_nodes_summary"));
        assert_eq!(fixture_title(&connection), "preserve-fixture-row");
    }

    #[test]
    fn migration_rejects_known_version_with_wrong_name() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");
        seed_partial_schema(&connection, "001", "wrong_init_name");

        let error = apply_migrations(&mut connection)
            .expect_err("mismatched migration name must be rejected");

        assert!(error.to_string().contains(
            "schema migration 001 name mismatch: expected 001_init, found wrong_init_name"
        ));
        assert!(!object_exists(&connection, "index", "idx_nodes_summary"));
        assert_eq!(fixture_title(&connection), "preserve-fixture-row");
    }

    #[test]
    fn migration_failure_rolls_back_schema_table_objects_and_markers() {
        const FAILING_MIGRATIONS: &[Migration] = &[
            Migration {
                version: "test001",
                name: "test001_create",
                sql: "CREATE TABLE migration_proof (id INTEGER PRIMARY KEY);",
            },
            Migration {
                version: "test002",
                name: "test002_fail",
                sql: "CREATE TABLE invalid_sql (",
            },
        ];
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");

        apply_migrations_from(&mut connection, FAILING_MIGRATIONS)
            .expect_err("invalid migration SQL should fail");

        assert!(!object_exists(&connection, "table", "schema_migrations"));
        assert!(!object_exists(&connection, "table", "migration_proof"));
        assert!(!object_exists(&connection, "table", "invalid_sql"));
        assert!(connection.is_autocommit());
    }

    fn seed_partial_schema(connection: &Connection, version: &str, name: &str) {
        connection
            .execute_batch(
                "
                CREATE TABLE schema_migrations (
                    version TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE nodes (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    node_type TEXT NOT NULL,
                    status TEXT NOT NULL,
                    title TEXT NOT NULL,
                    summary TEXT,
                    body TEXT,
                    source_ref TEXT,
                    confidence REAL,
                    trust_level TEXT,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO nodes (node_type, status, title)
                VALUES ('raw_note', 'draft', 'preserve-fixture-row');
                ",
            )
            .expect("partial schema should initialize");
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2);",
                (version, name),
            )
            .expect("migration marker should seed");
    }

    fn object_exists(connection: &Connection, object_type: &str, name: &str) -> bool {
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = ?1 AND name = ?2);",
                (object_type, name),
                |row| row.get(0),
            )
            .expect("sqlite_master should be readable")
    }

    fn fixture_title(connection: &Connection) -> String {
        connection
            .query_row("SELECT title FROM nodes WHERE id = 1;", [], |row| {
                row.get(0)
            })
            .expect("fixture row should remain readable")
    }

    #[test]
    fn apply_migrations_creates_nodes_table() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");

        apply_migrations(&mut connection).expect("migrations should apply");

        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'nodes';",
                [],
                |row| row.get(0),
            )
            .expect("sqlite_master should be queryable");

        assert_eq!(count, 1);
    }

    #[test]
    fn apply_migrations_creates_links_table() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");

        apply_migrations(&mut connection).expect("migrations should apply");

        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'links';",
                [],
                |row| row.get(0),
            )
            .expect("sqlite_master should be queryable");

        assert_eq!(count, 1);
    }

    #[test]
    fn apply_migrations_creates_aliases_tags_and_sources_tables() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");

        apply_migrations(&mut connection).expect("migrations should apply");

        for table in ["aliases", "tags", "sources"] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1;",
                    [table],
                    |row| row.get(0),
                )
                .expect("sqlite_master should be queryable");

            assert_eq!(count, 1, "{table} table should exist");
        }
    }

    #[test]
    fn apply_migrations_creates_events_table() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");

        apply_migrations(&mut connection).expect("migrations should apply");

        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'events';",
                [],
                |row| row.get(0),
            )
            .expect("sqlite_master should be queryable");
        let required_columns: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM pragma_table_info('events')
                WHERE name IN ('type', 'timestamp', 'source');
                ",
                [],
                |row| row.get(0),
            )
            .expect("events columns should be queryable");

        assert_eq!(count, 1);
        assert_eq!(required_columns, 3);
    }

    #[test]
    fn apply_migrations_creates_registry_base_tables() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");

        apply_migrations(&mut connection).expect("migrations should apply");

        for table in ["registries", "tool_contracts", "mcp_profiles"] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1;",
                    [table],
                    |row| row.get(0),
                )
                .expect("sqlite_master should be queryable");

            assert_eq!(count, 1, "{table} table should exist");
        }
    }

    #[test]
    fn apply_migrations_creates_fts_nodes_table() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for migration test");

        apply_migrations(&mut connection).expect("migrations should apply");

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'fts_nodes';",
                [],
                |row| row.get(0),
            )
            .expect("sqlite_master should be queryable");
        let column_count: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM pragma_table_info('fts_nodes')
                WHERE name IN ('title', 'summary', 'body', 'aliases');
                ",
                [],
                |row| row.get(0),
            )
            .expect("fts_nodes columns should be queryable");

        assert_eq!(table_count, 1);
        assert_eq!(column_count, 4);
    }
}
