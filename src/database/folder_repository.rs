//! Repository folder musik yang dipindai.

use std::path::Path;
use std::time::SystemTime;

use rusqlite::{params, Connection, Row};

use crate::core::model::LibraryFolder;
use crate::core::time::{from_unix_seconds, to_unix_seconds};

/// Repository folder di atas koneksi database.
pub struct FolderRepository<'a> {
    conn: &'a Connection,
}

impl<'a> FolderRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn row_to_folder(row: &Row<'_>) -> rusqlite::Result<LibraryFolder> {
        let path: String = row.get("path")?;
        let last_scanned_at: Option<i64> = row.get("last_scanned_at")?;

        Ok(LibraryFolder {
            id: row.get("id")?,
            path: Path::new(&path).to_path_buf(),
            enabled: row.get::<_, i64>("enabled")? != 0,
            last_scanned_at: last_scanned_at.and_then(from_unix_seconds),
        })
    }

    /// Daftar seluruh folder terdaftar.
    pub fn list(&self) -> rusqlite::Result<Vec<LibraryFolder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, enabled, last_scanned_at FROM library_folders ORDER BY path ASC",
        )?;
        let rows = stmt.query_map([], Self::row_to_folder)?;

        let mut folders = Vec::new();
        for row in rows {
            folders.push(row?);
        }
        Ok(folders)
    }

    /// Daftar folder yang aktif saja; folder dinonaktifkan dilewati saat pemindaian.
    pub fn list_enabled(&self) -> rusqlite::Result<Vec<LibraryFolder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, enabled, last_scanned_at FROM library_folders
             WHERE enabled = 1 ORDER BY path ASC",
        )?;
        let rows = stmt.query_map([], Self::row_to_folder)?;

        let mut folders = Vec::new();
        for row in rows {
            folders.push(row?);
        }
        Ok(folders)
    }

    /// Daftarkan folder baru; bila sudah ada, aktifkan kembali.
    pub fn add(&self, path: &Path) -> rusqlite::Result<i64> {
        let path = path.to_string_lossy().to_string();
        self.conn.execute(
            "INSERT INTO library_folders (path, enabled) VALUES (?1, 1)
             ON CONFLICT(path) DO UPDATE SET enabled = 1",
            params![path],
        )?;

        let id: i64 = self.conn.query_row(
            "SELECT id FROM library_folders WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    /// Aktifkan atau nonaktifkan folder.
    pub fn set_enabled(&self, id: i64, enabled: bool) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE library_folders SET enabled = ?2 WHERE id = ?1",
            params![id, i64::from(enabled)],
        )?;
        Ok(())
    }

    /// Hapus folder dari daftar; track yang sudah terindeks tetap ada.
    pub fn remove(&self, id: i64) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM library_folders WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Catat waktu pemindaian terakhir sebuah folder.
    pub fn mark_scanned(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE library_folders SET last_scanned_at = ?2 WHERE id = ?1",
            params![id, to_unix_seconds(SystemTime::now())],
        )?;
        Ok(())
    }
}
