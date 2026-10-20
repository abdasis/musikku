//! Repository state playback dan antrean aktif.

use rusqlite::{params, Connection};

use crate::core::model::PlaybackState;

/// Repository playback di atas koneksi database.
pub struct PlaybackRepository<'a> {
    conn: &'a Connection,
}

impl<'a> PlaybackRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Baca state playback tersimpan; mengembalikan default bila belum pernah disimpan.
    pub fn load_state(&self) -> rusqlite::Result<PlaybackState> {
        let mut state = PlaybackState::default();

        let result = self.conn.query_row(
            "SELECT current_track_id, position_ms, volume, muted, shuffle_enabled, repeat_mode
             FROM playback_state WHERE id = 1",
            [],
            |row| {
                state.current_track_id = row.get("current_track_id")?;
                state.position_ms = row.get("position_ms")?;
                state.volume = row.get("volume")?;
                state.muted = row.get::<_, i64>("muted")? != 0;
                state.shuffle_enabled = row.get::<_, i64>("shuffle_enabled")? != 0;
                state.repeat_mode = row
                    .get::<_, String>("repeat_mode")?
                    .parse()
                    .unwrap_or_default();
                Ok(())
            },
        );

        match result {
            Ok(()) => {}
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(err) => return Err(err),
        }

        state.queue_order = self.load_queue()?;
        Ok(state)
    }

    /// Simpan state playback.
    pub fn save_state(&self, state: &PlaybackState) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO playback_state
                 (id, current_track_id, position_ms, volume, muted, shuffle_enabled, repeat_mode)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                current_track_id = excluded.current_track_id,
                position_ms = excluded.position_ms,
                volume = excluded.volume,
                muted = excluded.muted,
                shuffle_enabled = excluded.shuffle_enabled,
                repeat_mode = excluded.repeat_mode",
            params![
                state.current_track_id,
                state.position_ms,
                state.volume,
                i64::from(state.muted),
                i64::from(state.shuffle_enabled),
                state.repeat_mode.as_str(),
            ],
        )?;
        Ok(())
    }

    /// Baca urutan antrean aktif.
    pub fn load_queue(&self) -> rusqlite::Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT track_id FROM queue_order ORDER BY position ASC")?;
        let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;

        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        Ok(ids)
    }

    /// Ganti urutan antrean aktif.
    pub fn save_queue(&self, track_ids: &[i64]) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM queue_order", [])?;
        for (position, track_id) in track_ids.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO queue_order (track_id, position) VALUES (?1, ?2)",
                params![track_id, position as i64],
            )?;
        }
        Ok(())
    }
}
