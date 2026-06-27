use rusqlite::Connection;

pub const MIGRATION_001_INIT: &str = "001_init";

struct Migration {
    version: &'static str,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
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
}];

pub fn apply_migrations(connection: &mut Connection) -> rusqlite::Result<()> {
    ensure_schema_migrations_table(connection)?;

    let transaction = connection.transaction()?;
    for migration in MIGRATIONS {
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "
            INSERT OR IGNORE INTO schema_migrations (version, name)
            VALUES (?1, ?2);
            ",
            (migration.version, migration.name),
        )?;
    }
    transaction.commit()
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

        assert_eq!(count, 1);
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
