//! State aplikasi yang dipakai bersama seluruh halaman UI.

use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::core::model::{Playlist, RepeatMode, Track};
use crate::database::{Database, PlaybackRepository, PlaylistRepository, SettingsRepository, TrackRepository};
use crate::library::LibraryService;
use crate::playback::{PlaybackController, PlaybackStatus};
use crate::settings::Settings;

/// Halaman yang bisa ditampilkan di area konten utama.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Home,
    Library,
    Playlists,
    Favorites,
    RecentlyPlayed,
    Settings,
}

impl Page {
    /// Judul halaman untuk header dan breadcrumb.
    pub fn title(self) -> &'static str {
        match self {
            Page::Home => "Home",
            Page::Library => "Library",
            Page::Playlists => "Playlist",
            Page::Favorites => "Favorit",
            Page::RecentlyPlayed => "Terakhir Diputar",
            Page::Settings => "Pengaturan",
        }
    }

    /// Ikon navigasi yang sesuai.
    pub fn icon_name(self) -> &'static str {
        match self {
            Page::Home => "go-home-symbolic",
            Page::Library => "view-list-symbolic",
            Page::Playlists => "view-grid-symbolic",
            Page::Favorites => "starred-symbolic",
            Page::RecentlyPlayed => "document-open-recent-symbolic",
            Page::Settings => "preferences-system-symbolic",
        }
    }

    /// Halaman yang sedang ditampilkan.
    pub fn current(self) -> Page {
        self
    }
}

/// Data library yang di-cache di memory agar UI tidak perlu query
/// database setiap kali menggambar ulang.
#[derive(Debug, Default, Clone)]
pub struct LibrarySnapshot {
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
    /// ID playlist yang sedang dibuka, bila ada.
    pub open_playlist_id: Option<i64>,
}

impl LibrarySnapshot {
    /// Cari track berdasarkan id.
    pub fn track(&self, id: i64) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Playlist yang sedang dibuka.
    pub fn open_playlist(&self) -> Option<&Playlist> {
        let id = self.open_playlist_id?;
        self.playlists.iter().find(|p| p.id == id)
    }
}

/// State aplikasi bersama.
pub struct AppState {
    pub database: Rc<Database>,
    pub library: LibraryService,
    pub playback: Arc<PlaybackController>,
    pub settings: Mutex<Settings>,
    /// Cache library.
    pub snapshot: Mutex<LibrarySnapshot>,
    /// Halaman aktif.
    pub page: Mutex<Page>,
    /// Pesan status terakhir yang ditampilkan sebagai toast.
    pub status_message: Mutex<Option<String>>,
    /// Pengirim perubahan halaman; UI mem-poll dari thread utama.
    page_tx: std::sync::mpsc::Sender<Page>,
    /// Penerima perubahan halaman.
    page_rx: std::sync::mpsc::Receiver<Page>,
    /// Permintaan memulai ulang pemindaian library.
    scan_tx: std::sync::mpsc::Sender<()>,
    /// Penerima permintaan pemindaian.
    scan_rx: std::sync::mpsc::Receiver<()>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").field("page", &self.page()).finish()
    }
}

impl AppState {
    pub fn new(
        database: Rc<Database>,
        library: LibraryService,
        playback: Arc<PlaybackController>,
        settings: Settings,
    ) -> Rc<Self> {
        let (page_tx, page_rx) = std::sync::mpsc::channel();
        let (scan_tx, scan_rx) = std::sync::mpsc::channel();

        Rc::new(Self {
            database,
            library,
            playback,
            settings: Mutex::new(settings),
            snapshot: Mutex::new(LibrarySnapshot::default()),
            page: Mutex::new(Page::Home),
            status_message: Mutex::new(None),
            page_tx,
            page_rx,
            scan_tx,
            scan_rx,
        })
    }

    /// Halaman aktif.
    pub fn page(&self) -> Page {
        *lock(&self.page)
    }

    /// Ubah halaman aktif dan beri tahu UI.
    pub fn navigate_to(&self, page: Page) {
        if self.page() == page {
            return;
        }
        *lock(&self.page) = page;
        let _ = self.page_tx.send(page);
    }

    /// Ambil perubahan halaman yang belum diproses.
    pub fn poll_page_change(&self) -> Option<Page> {
        self.page_rx.try_recv().ok()
    }

    /// Minta pemindaian library dimulai ulang.
    pub fn request_scan(&self) {
        let _ = self.scan_tx.send(());
    }

    /// Ambil permintaan pemindaian yang belum diproses.
    pub fn poll_scan_request(&self) -> bool {
        self.scan_rx.try_recv().is_ok()
    }

    /// State kosong, dipakai sebelum aplikasi benar-benar siap.
    pub fn placeholder() -> Self {
        let database = Rc::new(Database::open_in_memory().expect("Gagal membuat database sementara"));
        let library = LibraryService::new(
            Rc::clone(&database),
            std::path::PathBuf::from("/tmp"),
        );
        let playback = PlaybackController::new(crate::playback::AudioEngine::new());
        let (page_tx, page_rx) = std::sync::mpsc::channel();
        let (scan_tx, scan_rx) = std::sync::mpsc::channel();

        Self {
            database,
            library,
            playback,
            settings: Mutex::new(Settings {
                theme: crate::settings::Theme::default(),
                default_volume: 0.8,
                restore_last_playback: true,
                scan_on_startup: false,
                repeat_mode: crate::core::model::RepeatMode::default(),
                shuffle: false,
                artwork_dir: std::path::PathBuf::from("/tmp"),
                database_path: std::path::PathBuf::from(":memory:"),
            }),
            snapshot: Mutex::new(LibrarySnapshot::default()),
            page: Mutex::new(Page::Home),
            status_message: Mutex::new(None),
            page_tx,
            page_rx,
            scan_tx,
            scan_rx,
        }
    }

    /// Ubah halaman aktif.
    pub fn set_page(&self, page: Page) {
        *lock(&self.page) = page;
    }

    /// Halaman yang sedang aktif, dibaca UI.
    pub fn current_page(&self) -> Page {
        self.page()
    }

    /// Snapshot library saat ini.
    pub fn snapshot(&self) -> LibrarySnapshot {
        lock(&self.snapshot).clone()
    }

    /// Muat ulang cache library dari database.
    pub fn refresh_library(&self) {
        let tracks = {
            let repo = TrackRepository::new(self.database.connection());
            repo.query(
                &crate::database::TrackFilter::default(),
                crate::database::TrackSort::Title,
                crate::database::SortDirection::Ascending,
            )
            .unwrap_or_default()
        };

        let playlists = {
            let repo = PlaylistRepository::new(self.database.connection());
            repo.list().unwrap_or_default()
        };

        let mut snapshot = lock(&self.snapshot);
        snapshot.tracks = tracks;
        snapshot.playlists = playlists;
    }

    /// Atur halaman playlist yang sedang dibuka.
    pub fn set_open_playlist(&self, id: Option<i64>) {
        lock(&self.snapshot).open_playlist_id = id;
    }

    /// Playlist yang sedang dibuka.
    pub fn open_playlist(&self) -> Option<Playlist> {
        lock(&self.snapshot).open_playlist().cloned()
    }

    /// Track-track milik sebuah playlist.
    pub fn playlist_tracks(&self, playlist_id: i64) -> Vec<Track> {
        let repo = PlaylistRepository::new(self.database.connection());
        let ids = repo.track_ids(playlist_id).unwrap_or_default();
        let track_repo = TrackRepository::new(self.database.connection());
        track_repo.find_by_ids(&ids).unwrap_or_default()
    }

    /// Tandai track sebagai sudah diputar.
    pub fn mark_played(&self, track_id: i64) {
        let repo = TrackRepository::new(self.database.connection());
        if let Err(err) = repo.mark_played(track_id) {
            log::warn!("Gagal menandai track {track_id} sudah diputar: {err}");
        }
    }

    /// Ubah status favorit sebuah track.
    pub fn set_favorite(&self, track_id: i64, favorite: bool) {
        let repo = TrackRepository::new(self.database.connection());
        if let Err(err) = repo.set_favorite(track_id, favorite) {
            log::warn!("Gagal mengubah status favorit track {track_id}: {err}");
        }
    }

    /// Tampilkan pesan status singkat.
    pub fn set_status_message(&self, message: impl Into<String>) {
        *lock(&self.status_message) = Some(message.into());
    }

    /// Ambil dan kosongkan pesan status.
    pub fn take_status_message(&self) -> Option<String> {
        lock(&self.status_message).take()
    }

    /// Kosongkan pesan status yang sedang tampil.
    pub fn clear_status_message(&self) {
        *lock(&self.status_message) = None;
    }

    /// Simpan preferensi yang sedang aktif.
    pub fn save_settings(&self) {
        let repo = SettingsRepository::new(self.database.connection());
        let settings = lock(&self.settings).clone();
        if let Err(err) = settings.save(&repo) {
            log::error!("Gagal menyimpan preferensi: {err}");
        }
    }

    /// Mode pengulangan aktif.
    pub fn repeat_mode(&self) -> RepeatMode {
        self.playback.repeat_mode()
    }

    /// Status pemutaran aktif.
    pub fn playback_status(&self) -> PlaybackStatus {
        self.playback.status()
    }

    /// Simpan state playback ke database.
    pub fn save_playback_state(&self) {
        let repo = PlaybackRepository::new(self.database.connection());
        let queue = self.playback.queue();

        let state = crate::core::model::PlaybackState {
            current_track_id: self.playback.current_track().map(|t| t.id),
            position_ms: self.playback.position_ms(),
            volume: self.playback.volume(),
            muted: self.playback.is_muted(),
            shuffle_enabled: self.playback.shuffle_enabled(),
            repeat_mode: self.playback.repeat_mode(),
            queue_order: queue.order().to_vec(),
        };

        if let Err(err) = repo.save_state(&state) {
            log::error!("Gagal menyimpan state playback: {err}");
        }
        if let Err(err) = repo.save_queue(&state.queue_order) {
            log::error!("Gagal menyimpan antrean: {err}");
        }
    }
}

/// Kunci mutex dan kembalikan nilainya; mengabaikan poisoning karena
/// tidak ada state yang tidak pulih setelah panic.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}
