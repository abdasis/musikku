//! Repository playlist: CRUD playlist dan urutan lagunya.

use std::time::SystemTime;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::core::model::Playlist;
use crate::core::time::{from_unix_seconds, to_unix_seconds};

/// Repository playlist di atas koneksi database.
pub struct PlaylistRepository<'a> {
    conn: &'a Connection,
}

impl<'a> PlaylistRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn row_to_playlist(row: &Row<'_>) -> rusqlite::Result<Playlist> {
        let created_at: i64 = row.get("created_at")?;
        let updated_at: i64 = row.get("updated_at")?;

        Ok(Playlist {
            id: row.get("id")?,
            name: row.get("name")?,
            created_at: from_unix_seconds(created_at).unwrap_or(SystemTime::UNIX_EPOCH),
            updated_at: from_unix_seconds(updated_at).unwrap_or(SystemTime::UNIX_EPOCH),
            track_count: row.get("track_count")?,
        })
    }

    /// Daftar seluruh playlist beserta jumlah lagunya.
    pub fn list(&self) -> rusqlite::Result<Vec<Playlist>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.name, p.created_at, p.updated_at,
                    (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id) AS track_count
             FROM playlists p
             ORDER BY p.name COLLATE NOCASE ASC",
        )?;
        let rows = stmt.query_map([], Self::row_to_playlist)?;

        let mut playlists = Vec::new();
        for row in rows {
            playlists.push(row?);
        }
        Ok(playlists)
    }

    /// Ambil satu playlist berdasarkan id.
    pub fn find_by_id(&self, id: i64) -> rusqlite::Result<Option<Playlist>> {
        self.conn
            .query_row(
                "SELECT p.id, p.name, p.created_at, p.updated_at,
                        (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id) AS track_count
                 FROM playlists p WHERE p.id = ?1",
                params![id],
                Self::row_to_playlist,
            )
            .optional()
    }

    /// Buat playlist baru dengan nama tertentu.
    pub fn create(&self, name: &str) -> rusqlite::Result<i64> {
        let now = to_unix_seconds(SystemTime::now());
        self.conn.execute(
            "INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
            params![name, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Ubah nama playlist.
    pub fn rename(&self, id: i64, name: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE playlists SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, name, to_unix_seconds(SystemTime::now())],
        )?;
        Ok(())
    }

    /// Hapus playlist. Berkas audio tidak ikut terhapus.
    pub fn remove(&self, id: i64) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Tambahkan track ke playlist pada posisi terakhir.
    pub fn add_track(&self, playlist_id: i64, track_id: i64) -> rusqlite::Result<()> {
        let next_position: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )?;

        self.conn.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
            params![playlist_id, track_id, next_position],
        )?;
        self.touch(playlist_id)?;
        Ok(())
    }

    /// Hapus track dari playlist, lalu rapatkan urutan.
    pub fn remove_track(&self, playlist_id: i64, track_id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            params![playlist_id, track_id],
        )?;
        self.compact_positions(playlist_id)?;
        self.touch(playlist_id)?;
        Ok(())
    }

    /// Susun ulang isi playlist mengikuti urutan track id yang diberikan.
    pub fn reorder(&self, playlist_id: i64, track_ids: &[i64]) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
        )?;

        for (position, track_id) in track_ids.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                params![playlist_id, track_id, position as i64],
            )?;
        }
        self.touch(playlist_id)?;
        Ok(())
    }

    /// Daftar id track dalam playlist sesuai urutannya.
    pub fn track_ids(&self, playlist_id: i64) -> rusqlite::Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position ASC",
        )?;
        let rows = stmt.query_map(params![playlist_id], |row| row.get::<_, i64>(0))?;

        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        Ok(ids)
    }

    /// Perbarui waktu modifikasi playlist.
    fn touch(&self, playlist_id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE playlists SET updated_at = ?2 WHERE id = ?1",
            params![playlist_id, to_unix_seconds(SystemTime::now())],
        )?;
        Ok(())
    }

    /// Rapatkan ulang nilai position agar berurutan tanpa celah.
    fn compact_positions(&self, playlist_id: i64) -> rusqlite::Result<()> {
        let ids = self.track_ids(playlist_id)?;
        self.conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
        )?;
        for (position, track_id) in ids.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                params![playlist_id, track_id, position as i64],
            )?;
        }
        Ok(())
    }
}
