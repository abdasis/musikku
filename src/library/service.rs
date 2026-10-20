//! Layanan library: menjalankan pemindaian di luar main thread.
//!
//! Thread pemindai hanya mengerjakan pembacaan berkas (bagian lambat),
//! lalu mengirim track yang berhasil dibaca ke main thread untuk disimpan.
//! Database tetap dipakai dari satu thread saja.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::core::model::{LibraryFolder, Track};
use crate::database::{Database, FolderRepository, TrackRepository};

use super::scanner::{ScanControl, ScanProgress, Scanner};

/// Pesan yang dikirim dari thread pemindai ke main thread.
#[derive(Debug)]
pub enum LibraryEvent {
    /// Pemindaian dimulai dengan jumlah total berkas.
    Started { total: usize },
    /// Track berhasil dibaca; main thread yang menyimpannya ke database.
    TrackFound(Box<Track>),
    /// Progres pemindaian.
    Progress(ScanProgress),
    /// Pemindaian selesai.
    Finished {
        added: usize,
        failed: usize,
        cancelled: bool,
    },
    /// Pemindaian gagal total.
    Failed(String),
}

/// Kendali pemindaian yang bisa dipakai dari main thread.
pub struct ScanHandle {
    control: ScanControl,
    events: Receiver<LibraryEvent>,
}

impl ScanHandle {
    /// Ambil event berikutnya bila ada, tanpa memblokir.
    pub fn poll(&self) -> Option<LibraryEvent> {
        self.events.try_recv().ok()
    }

    /// Batalkan pemindaian yang sedang berjalan.
    pub fn cancel(&self) {
        self.control.cancel();
    }
}

/// Layanan library.
pub struct LibraryService {
    database: Rc<Database>,
    artwork_dir: PathBuf,
}

impl LibraryService {
    pub fn new(database: Rc<Database>, artwork_dir: PathBuf) -> Self {
        Self {
            database,
            artwork_dir,
        }
    }

    pub fn database(&self) -> &Rc<Database> {
        &self.database
    }

    /// Simpan satu track hasil pemindaian ke database.
    pub fn persist_track(&self, track: &Track) -> bool {
        let repo = TrackRepository::new(self.database.connection());
        match repo.upsert(track) {
            Ok(_) => true,
            Err(err) => {
                log::warn!("Gagal menyimpan track {}: {err}", track.file_path.display());
                false
            }
        }
    }

    /// Daftar folder musik yang terdaftar.
    pub fn folders(&self) -> Vec<LibraryFolder> {
        let repo = FolderRepository::new(self.database.connection());
        repo.list().unwrap_or_default()
    }

    /// Daftarkan folder musik baru.
    pub fn add_folder(&self, path: &std::path::Path) -> rusqlite::Result<i64> {
        let repo = FolderRepository::new(self.database.connection());
        repo.add(path)
    }

    /// Hapus folder dari daftar.
    pub fn remove_folder(&self, id: i64) -> rusqlite::Result<()> {
        let repo = FolderRepository::new(self.database.connection());
        repo.remove(id)
    }

    /// Aktifkan atau nonaktifkan folder.
    pub fn set_folder_enabled(&self, id: i64, enabled: bool) -> rusqlite::Result<()> {
        let repo = FolderRepository::new(self.database.connection());
        repo.set_enabled(id, enabled)
    }

    /// Jalankan pemindaian seluruh folder aktif di thread terpisah.
    pub fn start_scan(&self) -> ScanHandle {
        let control = ScanControl::new();
        let (tx, rx) = channel::<LibraryEvent>();

        let folders = {
            let repo = FolderRepository::new(self.database.connection());
            repo.list_enabled().unwrap_or_default()
        };

        let known_paths = {
            let repo = TrackRepository::new(self.database.connection());
            repo.existing_paths().unwrap_or_default()
        };

        let artwork_dir = self.artwork_dir.clone();
        let scan_control = control.clone();

        std::thread::spawn(move || {
            let scanner = Scanner::new(artwork_dir);

            let mut all_files = Vec::new();
            for folder in &folders {
                all_files.extend(Scanner::collect_files(&folder.path));
            }
            let total = all_files.len();

            if tx.send(LibraryEvent::Started { total }).is_err() {
                return;
            }

            let mut added = 0usize;
            let mut failed = 0usize;
            let mut cancelled = false;

            for folder in &folders {
                if scan_control.is_cancelled() {
                    cancelled = true;
                    break;
                }

                let result = scanner.scan_folder(&folder.path, &scan_control, &known_paths);
                failed += result.failed.len();

                for track in result.tracks {
                    if tx.send(LibraryEvent::TrackFound(Box::new(track))).is_err() {
                        return;
                    }
                    added += 1;
                }

                let _ = tx.send(LibraryEvent::Progress(ScanProgress {
                    processed: added + failed,
                    total,
                    current_file: None,
                }));
            }

            let _ = tx.send(LibraryEvent::Finished {
                added,
                failed,
                cancelled,
            });
        });

        ScanHandle {
            control,
            events: rx,
        }
    }

    /// Catat waktu pemindaian terakhir untuk folder yang aktif.
    pub fn mark_folders_scanned(&self) {
        let repo = FolderRepository::new(self.database.connection());
        if let Ok(folders) = repo.list_enabled() {
            for folder in folders {
                let _ = repo.mark_scanned(folder.id);
            }
        }
    }

    /// Ambil seluruh track dalam library.
    pub fn all_tracks(&self) -> Vec<Track> {
        let repo = TrackRepository::new(self.database.connection());
        repo.query(
            &crate::database::TrackFilter::default(),
            crate::database::TrackSort::Title,
            crate::database::SortDirection::Ascending,
        )
        .unwrap_or_default()
    }
}

/// Pengirim event ke UI.
pub type EventSender = Sender<LibraryEvent>;
