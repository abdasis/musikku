//! Migrasi skema database.
//!
//! Migrasi dijalankan berurutan dan dicatat di tabel `schema_migrations`,
//! sehingga aplikasi aman dibuka berulang kali.

use rusqlite::Connection;

/// Migrasi yang akan dijalankan, berurutan dari versi lama ke baru.
const MIGRATIONS: &[&str] = &[
    // v1: tabel inti library, playlist, dan state playback.
    r#"
    CREATE TABLE IF NOT EXISTS tracks (
        id                INTEGER PRIMARY KEY AUTOINCREMENT,
        file_path         TEXT    NOT NULL UNIQUE,
        title             TEXT    NOT NULL DEFAULT '',
        artist            TEXT    NOT NULL DEFAULT '',
        album             TEXT    NOT NULL DEFAULT '',
        album_artist      TEXT    NOT NULL DEFAULT '',
        duration_ms       INTEGER NOT NULL DEFAULT 0,
        track_number      INTEGER,
        disc_number       INTEGER,
        year              INTEGER,
        genre             TEXT,
        artwork_path      TEXT,
        file_size         INTEGER NOT NULL DEFAULT 0,
        file_modified_at  INTEGER,
        scan_status       TEXT    NOT NULL DEFAULT 'ready',
        last_played_at    INTEGER,
        play_count        INTEGER NOT NULL DEFAULT 0,
        is_favorite       INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE IF NOT EXISTS library_folders (
        id               INTEGER PRIMARY KEY AUTOINCREMENT,
        path             TEXT    NOT NULL UNIQUE,
        enabled          INTEGER NOT NULL DEFAULT 1,
        last_scanned_at  INTEGER
    );

    CREATE TABLE IF NOT EXISTS playlists (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        name        TEXT    NOT NULL,
        created_at  INTEGER NOT NULL,
        updated_at  INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS playlist_tracks (
        playlist_id  INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
        track_id     INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        position     INTEGER NOT NULL,
        PRIMARY KEY (playlist_id, position)
    );

    CREATE TABLE IF NOT EXISTS playback_state (
        id                 INTEGER PRIMARY KEY CHECK (id = 1),
        current_track_id   INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
        position_ms        INTEGER NOT NULL DEFAULT 0,
        volume             REAL    NOT NULL DEFAULT 0.8,
        muted              INTEGER NOT NULL DEFAULT 0,
        shuffle_enabled    INTEGER NOT NULL DEFAULT 0,
        repeat_mode        TEXT    NOT NULL DEFAULT 'off'
    );

    CREATE TABLE IF NOT EXISTS queue_order (
        track_id  INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
        position  INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS settings (
        key    TEXT PRIMARY KEY,
        value  TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_tracks_title    ON tracks(title);
    CREATE INDEX IF NOT EXISTS idx_tracks_artist   ON tracks(artist);
    CREATE INDEX IF NOT EXISTS idx_tracks_album    ON tracks(album);
    CREATE INDEX IF NOT EXISTS idx_tracks_favorite ON tracks(is_favorite);
    CREATE INDEX IF NOT EXISTS idx_tracks_played   ON tracks(last_played_at);
    CREATE INDEX IF NOT EXISTS idx_playlist_tracks ON playlist_tracks(playlist_id, position);
    CREATE INDEX IF NOT EXISTS idx_queue_position  ON queue_order(position);
    "#,
];

/// Jalankan seluruh migrasi yang belum tercatat.
pub fn run(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;

    for (index, sql) in MIGRATIONS.iter().enumerate() {
        let version = index as i64 + 1;
        if version <= current {
            continue;
        }

        conn.execute_batch(sql)?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![
                version,
                crate::core::time::to_unix_seconds(std::time::SystemTime::now())
            ],
        )?;
        log::info!("Migrasi database versi {version} selesai");
    }

    Ok(())
}
