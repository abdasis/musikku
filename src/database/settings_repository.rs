//! Repository preferensi pengguna berbasis tabel `settings`.

use rusqlite::{params, Connection, OptionalExtension};

/// Repository settings di atas koneksi database.
pub struct SettingsRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SettingsRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Baca nilai preferensi sebagai teks.
    pub fn get(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
    }

    /// Simpan nilai preferensi.
    pub fn set(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Baca nilai preferensi sebagai boolean.
    pub fn get_bool(&self, key: &str, default: bool) -> rusqlite::Result<bool> {
        Ok(match self.get(key)? {
            Some(value) => value == "true" || value == "1",
            None => default,
        })
    }

    /// Simpan nilai preferensi sebagai boolean.
    pub fn set_bool(&self, key: &str, value: bool) -> rusqlite::Result<()> {
        self.set(key, if value { "true" } else { "false" })
    }

    /// Baca nilai preferensi sebagai bilangan pecahan.
    pub fn get_f64(&self, key: &str, default: f64) -> rusqlite::Result<f64> {
        Ok(self
            .get(key)?
            .and_then(|v| v.parse().ok())
            .unwrap_or(default))
    }

    /// Simpan nilai preferensi sebagai bilangan pecahan.
    pub fn set_f64(&self, key: &str, value: f64) -> rusqlite::Result<()> {
        self.set(key, &value.to_string())
    }
}
