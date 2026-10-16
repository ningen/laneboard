use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

use crate::error::Result;

pub fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("board")
        .join("board.db")
}

pub fn open_db(path: impl AsRef<Path>) -> Result<Connection> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| crate::Error::msg(e.to_string()))?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        ",
    )?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS cards (
            id TEXT PRIMARY KEY,
            key TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            body TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL,
            labels_json TEXT NOT NULL DEFAULT '[]',
            assignee_kind TEXT NOT NULL DEFAULT 'unassigned',
            assignee_name TEXT,
            priority INTEGER NOT NULL DEFAULT 0,
            external_url TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_cards_status ON cards(status);
        CREATE INDEX IF NOT EXISTS idx_cards_assignee ON cards(assignee_kind);

        CREATE TABLE IF NOT EXISTS card_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_id TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
            to_id TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
            link_type TEXT NOT NULL,
            note TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(from_id, to_id, link_type)
        );

        CREATE INDEX IF NOT EXISTS idx_links_from ON card_links(from_id);
        CREATE INDEX IF NOT EXISTS idx_links_to ON card_links(to_id);

        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            at TEXT NOT NULL,
            actor_kind TEXT NOT NULL,
            actor_name TEXT,
            card_id TEXT REFERENCES cards(id) ON DELETE SET NULL,
            kind TEXT NOT NULL,
            payload_json TEXT NOT NULL DEFAULT '{}'
        );

        CREATE INDEX IF NOT EXISTS idx_events_card ON events(card_id, at DESC);
        CREATE INDEX IF NOT EXISTS idx_events_at ON events(at DESC);

        CREATE VIRTUAL TABLE IF NOT EXISTS cards_fts USING fts5(
            key,
            title,
            body,
            labels,
            content='cards',
            content_rowid='rowid'
        );
        "#,
    )?;

    // Keep FTS in sync via triggers (idempotent create)
    let has_trigger: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='trigger' AND name='cards_ai'",
            [],
            |r| r.get(0),
        )
        .optional()?;

    if has_trigger.is_none() {
        conn.execute_batch(
            r#"
            CREATE TRIGGER cards_ai AFTER INSERT ON cards BEGIN
              INSERT INTO cards_fts(rowid, key, title, body, labels)
              VALUES (new.rowid, new.key, new.title, new.body, new.labels_json);
            END;

            CREATE TRIGGER cards_ad AFTER DELETE ON cards BEGIN
              INSERT INTO cards_fts(cards_fts, rowid, key, title, body, labels)
              VALUES('delete', old.rowid, old.key, old.title, old.body, old.labels_json);
            END;

            CREATE TRIGGER cards_au AFTER UPDATE ON cards BEGIN
              INSERT INTO cards_fts(cards_fts, rowid, key, title, body, labels)
              VALUES('delete', old.rowid, old.key, old.title, old.body, old.labels_json);
              INSERT INTO cards_fts(rowid, key, title, body, labels)
              VALUES (new.rowid, new.key, new.title, new.body, new.labels_json);
            END;
            "#,
        )?;
    }

    let version: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if version.is_none() {
        conn.execute(
            "INSERT INTO meta(key, value) VALUES ('schema_version', '1')",
            [],
        )?;
    }

    migrate_v2(conn)?;

    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| {
        let name: String = row.get(1)?;
        Ok(name)
    })?;
    for row in rows {
        if row? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn migrate_v2(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "cards", "claim_expires_at")? {
        conn.execute("ALTER TABLE cards ADD COLUMN claim_expires_at TEXT", [])?;
    }
    conn.execute(
        "INSERT INTO meta(key, value) VALUES ('schema_version', '2')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [],
    )?;
    Ok(())
}