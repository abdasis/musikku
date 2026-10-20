//! Preferensi pengguna: tema, direktori musik, dan perilaku playback.

use std::path::PathBuf;
use std::str::FromStr;

use crate::core::model::RepeatMode;
use crate::database::SettingsRepository;

/// Kunci preferensi yang disimpan di database.
mod keys {
    pub const THEME: &str = "appearance.theme";
    pub const DEFAULT_VOLUME: &str = "playback.default_volume";
    pub const RESTORE_LAST_PLAYBACK: &str = "general.restore_last_playback";
    pub const SCAN_ON_STARTUP: &str = "library.scan_on_startup";
    pub const REPEAT_MODE: &str = "playback.repeat_mode";
    pub const SHUFFLE: &str = "playback.shuffle";
}

/// Tema tampilan aplikasi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// Ikuti tema sistem.
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    /// Nama tema untuk ditampilkan di pengaturan.
    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "Ikuti Sistem",
            Theme::Light => "Terang",
            Theme::Dark => "Gelap",
        }
    }
}

impl FromStr for Theme {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        })
    }
}

/// Semua preferensi aplikasi.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub theme: Theme,
    pub default_volume: f64,
    pub restore_last_playback: bool,
    pub scan_on_startup: bool,
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,
    /// Folder tempat cover album disimpan.
    pub artwork_dir: PathBuf,
    /// Lokasi berkas database.
    pub database_path: PathBuf,
}

impl Settings {
    /// Baca preferensi dari database, memakai nilai default bila belum ada.
    pub fn load(repo: &SettingsRepository, artwork_dir: PathBuf, database_path: PathBuf) -> Self {
        let theme = repo
            .get(keys::THEME)
            .ok()
            .flatten()
            .map(|value| value.parse().unwrap_or_default())
            .unwrap_or_default();

        let default_volume = repo
            .get_f64(keys::DEFAULT_VOLUME, 0.8)
            .unwrap_or(0.8)
            .clamp(0.0, 1.0);

        let repeat_mode = repo
            .get(keys::REPEAT_MODE)
            .ok()
            .flatten()
            .map(|value| value.parse().unwrap_or_default())
            .unwrap_or_default();

        Self {
            theme,
            default_volume,
            restore_last_playback: repo
                .get_bool(keys::RESTORE_LAST_PLAYBACK, true)
                .unwrap_or(true),
            scan_on_startup: repo.get_bool(keys::SCAN_ON_STARTUP, false).unwrap_or(false),
            repeat_mode,
            shuffle: repo.get_bool(keys::SHUFFLE, false).unwrap_or(false),
            artwork_dir,
            database_path,
        }
    }

    /// Simpan preferensi ke database.
    pub fn save(&self, repo: &SettingsRepository) -> rusqlite::Result<()> {
        repo.set(keys::THEME, self.theme.as_str())?;
        repo.set_f64(keys::DEFAULT_VOLUME, self.default_volume)?;
        repo.set_bool(keys::RESTORE_LAST_PLAYBACK, self.restore_last_playback)?;
        repo.set_bool(keys::SCAN_ON_STARTUP, self.scan_on_startup)?;
        repo.set(keys::REPEAT_MODE, self.repeat_mode.as_str())?;
        repo.set_bool(keys::SHUFFLE, self.shuffle)?;
        Ok(())
    }
}
