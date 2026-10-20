//! Pemindai folder musik.
//!
//! Pemindaian berjalan di thread terpisah agar GTK main thread tidak
//! terblokir. Scanner dapat dibatalkan, melaporkan progres, dan tahan
//! terhadap berkas rusak maupun folder yang tidak dapat diakses.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::core::model::{ScanStatus, Track};

/// Ekstensi berkas audio yang dipindai.
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "oga", "opus", "m4a", "aac", "wma", "aiff", "aif", "ape", "mpc",
    "wv", "opus",
];

/// Progress pemindaian yang dilaporkan ke UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProgress {
    /// Jumlah berkas yang sudah diproses.
    pub processed: usize,
    /// Total berkas yang ditemukan.
    pub total: usize,
    /// Berkas yang sedang diproses.
    pub current_file: Option<PathBuf>,
}

/// Hasil pemindaian satu folder.
#[derive(Debug, Default)]
pub struct ScanResult {
    /// Track yang berhasil dibaca metadatanya.
    pub tracks: Vec<Track>,
    /// Path berkas yang tidak bisa dibaca.
    pub failed: Vec<(PathBuf, String)>,
    /// Jumlah track yang sudah terindeks sebelumnya dan tidak berubah.
    pub unchanged: usize,
}

/// Kendali pembatalan dan progress pemindaian.
#[derive(Clone, Default)]
pub struct ScanControl {
    cancelled: Arc<AtomicBool>,
    processed: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
}

impl ScanControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Minta pemindaian dihentikan. Pemindaian berhenti pada berkas berikutnya.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Apakah pemindaian sudah dibatalkan.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Progress terakhir yang dilaporkan.
    pub fn progress(&self) -> ScanProgress {
        ScanProgress {
            processed: self.processed.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
            current_file: None,
        }
    }

    fn set_total(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
    }

    fn increment_processed(&self) {
        self.processed.fetch_add(1, Ordering::Relaxed);
    }
}

/// Pemindai folder musik.
pub struct Scanner {
    /// Direktori tempat cover album disimpan.
    artwork_dir: PathBuf,
}

impl Scanner {
    pub fn new(artwork_dir: PathBuf) -> Self {
        Self { artwork_dir }
    }

    /// whether path extension is a supported audio format.
    fn is_audio_file(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.as_str()))
    }

    /// Kumpulkan seluruh berkas audio di dalam folder secara rekursif.
    ///
    /// Folder yang tidak dapat diakses dilewati tanpa menghentikan pemindaian.
    pub fn collect_files(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];

        while let Some(dir) = stack.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(err) => {
                    log::warn!("Tidak dapat membaca folder {}: {err}", dir.display());
                    continue;
                }
            };

            for entry in entries.flatten() {
                let path = entry.path();
                let file_type = match entry.file_type() {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                if file_type.is_dir() {
                    stack.push(path);
                } else if file_type.is_file() && Self::is_audio_file(&path) {
                    files.push(path);
                }
            }
        }

        files.sort();
        files
    }

    /// Pindai satu folder dan kembalikan track yang ditemukan.
    ///
    /// `known_paths` berisi path yang sudah terindeks; track lama tidak
    /// dibaca ulang sehingga pemindaian berikutnya jauh lebih cepat.
    pub fn scan_folder(
        &self,
        root: &Path,
        control: &ScanControl,
        known_paths: &std::collections::HashSet<String>,
    ) -> ScanResult {
        let mut result = ScanResult::default();

        if !root.exists() {
            log::warn!("Folder musik tidak ditemukan: {}", root.display());
            return result;
        }

        let files = Self::collect_files(root);
        control.set_total(files.len());

        for path in files {
            if control.is_cancelled() {
                log::info!("Pemindaian dibatalkan pada {}", path.display());
                break;
            }

            let path_string = path.to_string_lossy().to_string();
            if known_paths.contains(&path_string) {
                result.unchanged += 1;
                control.increment_processed();
                continue;
            }

            match self.scan_file(&path) {
                Ok(track) => {
                    result.tracks.push(track);
                }
                Err(err) => {
                    log::warn!("Gagal membaca {}: {err}", path.display());
                    result.failed.push((path, err.to_string()));
                }
            }

            control.increment_processed();
        }

        result
    }

    /// Baca metadata satu berkas dan simpan cover album bila ada.
    pub fn scan_file(&self, path: &Path) -> Result<Track, lofty::error::FileParseError> {
        let metadata = super::metadata::read_metadata(path)?;

        let artwork_path = metadata.artwork.as_ref().and_then(|data| {
            super::metadata::save_artwork(data, metadata.artwork_mime.as_deref(), &self.artwork_dir)
                .ok()
        });

        Ok(super::metadata::build_track(path, &metadata, artwork_path))
    }

    /// Periksa apakah track yang sudah terindeks masih ada di disk.
    pub fn is_file_alive(track: &Track) -> bool {
        track.file_path.is_file() && track.scan_status != ScanStatus::Missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_extension_is_detected_case_insensitively() {
        assert!(Scanner::is_audio_file(Path::new("/music/song.MP3")));
        assert!(Scanner::is_audio_file(Path::new("/music/song.flac")));
        assert!(!Scanner::is_audio_file(Path::new("/music/cover.jpg")));
        assert!(!Scanner::is_audio_file(Path::new("/music/song")));
    }

    #[test]
    fn collect_files_skips_missing_directory() {
        let files = Scanner::collect_files(Path::new("/tidak/ada/path/ini"));
        assert!(files.is_empty());
    }
}
