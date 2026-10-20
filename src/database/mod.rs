//! Koneksi, skema, dan repository database.

pub mod folder_repository;
pub mod migration;
pub mod playback_repository;
pub mod playlist_repository;
pub mod settings_repository;
pub mod track_repository;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

pub use folder_repository::FolderRepository;
pub use playback_repository::PlaybackRepository;
pub use playlist_repository::PlaylistRepository;
pub use settings_repository::SettingsRepository;
pub use track_repository::{SortDirection, TrackFilter, TrackRepository, TrackSort};

/// Envolusi koneksi database dengan lokasi berkas yang diketahui.
pub struct Database {
    conn: Connection,
    path: PathBuf,
}

impl Database {
    /// Buka (atau buat) database di `path` dan jalankan migrasi.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        }

        let conn = Connection::open(path)?;
        Self::configure(&conn)?;
        migration::run(&conn)?;

        Ok(Self {
            conn,
            path: path.to_path_buf(),
        })
    }

    /// Buka database in-memory, dipakai pada pengujian.
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure(&conn)?;
        migration::run(&conn)?;

        Ok(Self {
            conn,
            path: PathBuf::from(":memory:"),
        })
    }

    fn configure(conn: &Connection) -> rusqlite::Result<()> {
        // WAL hanya berlaku untuk database berbasis berkas.
        if !conn.path().unwrap_or_default().contains(":memory:") {
            conn.pragma_update(None, "journal_mode", "WAL")?;
        }
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Ok(())
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
