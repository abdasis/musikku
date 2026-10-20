//! Domain model: entitas data yang dipakai seluruh aplikasi.

use std::path::PathBuf;
use std::str::FromStr;
use std::time::SystemTime;

/// Status pemindaian sebuah track terhadap file di disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScanStatus {
    /// Track sudah terindeks dan file masih ada.
    #[default]
    Ready,
    /// File hilang atau tidak bisa diakses.
    Missing,
    /// File ada tapi tidak bisa di-decode.
    Unreadable,
}

impl ScanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ScanStatus::Ready => "ready",
            ScanStatus::Missing => "missing",
            ScanStatus::Unreadable => "unreadable",
        }
    }
}

impl FromStr for ScanStatus {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "missing" => ScanStatus::Missing,
            "unreadable" => ScanStatus::Unreadable,
            _ => ScanStatus::Ready,
        })
    }
}

/// Metadata satu berkas audio.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: i64,
    pub file_path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub duration_ms: i64,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub artwork_path: Option<PathBuf>,
    pub file_size: i64,
    pub file_modified_at: Option<SystemTime>,
    pub scan_status: ScanStatus,
    /// Recently played, dipakai untuk halaman Home.
    pub last_played_at: Option<SystemTime>,
    pub play_count: i64,
    /// Ditandai sebagai favorit.
    pub is_favorite: bool,
}

impl Track {
    /// Judul siap tampil; jatuh ke nama berkas tanpa ekstensi bila tag kosong.
    pub fn display_title(&self) -> &str {
        if self.title.trim().is_empty() {
            self.file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Lagu Tanpa Judul")
        } else {
            &self.title
        }
    }

    /// Artis siap tampil; jatuh ke "Artis Tidak Diketahui" bila tag kosong.
    pub fn display_artist(&self) -> &str {
        if self.artist.trim().is_empty() {
            "Artis Tidak Diketahui"
        } else {
            &self.artist
        }
    }

    /// Album siap tampil; jatuh ke "Album Tidak Diketahui" bila tag kosong.
    pub fn display_album(&self) -> &str {
        if self.album.trim().is_empty() {
            "Album Tidak Diketahui"
        } else {
            &self.album
        }
    }
}

/// Playlist pengguna beserta jumlah lagunya.
#[derive(Debug, Clone, PartialEq)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub track_count: i64,
}

/// Folder musik yang dipindai.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryFolder {
    pub id: i64,
    pub path: PathBuf,
    pub enabled: bool,
    pub last_scanned_at: Option<SystemTime>,
}

/// Mode pengulangan playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    /// Lagu berikutnya diputar setelah lagu terakhir selesai.
    #[default]
    Off,
    /// Setelah lagu terakhir, kembali ke lagu pertama.
    All,
    /// Lagu yang sama diulang terus-menerus.
    One,
}

impl RepeatMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RepeatMode::Off => "off",
            RepeatMode::All => "all",
            RepeatMode::One => "one",
        }
    }

    /// Ikon yang sesuai untuk mode aktif.
    pub fn icon_name(self) -> &'static str {
        match self {
            RepeatMode::Off => "media-playlist-repeat-symbolic",
            RepeatMode::All => "media-playlist-repeat",
            RepeatMode::One => "media-playlist-repeat-one",
        }
    }
}

impl FromStr for RepeatMode {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "all" => RepeatMode::All,
            "one" => RepeatMode::One,
            _ => RepeatMode::Off,
        })
    }
}

/// Keadaan playback yang tersimpan di database.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackState {
    pub current_track_id: Option<i64>,
    pub position_ms: i64,
    pub volume: f64,
    pub muted: bool,
    pub shuffle_enabled: bool,
    pub repeat_mode: RepeatMode,
    /// Urutan track id dalam antrean aktif.
    pub queue_order: Vec<i64>,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            current_track_id: None,
            position_ms: 0,
            volume: 0.8,
            muted: false,
            shuffle_enabled: false,
            repeat_mode: RepeatMode::Off,
            queue_order: Vec::new(),
        }
    }
}

/// Album yang dikelompokkan dari track untuk halaman album.
#[derive(Debug, Clone, PartialEq)]
pub struct Album {
    pub name: String,
    pub artist: String,
    pub year: Option<u32>,
    pub artwork_path: Option<PathBuf>,
    pub track_count: i64,
    pub total_duration_ms: i64,
    pub is_favorite: bool,
}
