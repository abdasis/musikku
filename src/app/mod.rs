//! Modul aplikasi: state bersama dan siklus hidup aplikasi.

pub mod state;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;

use crate::database::{Database, PlaybackRepository, SettingsRepository, TrackRepository};
use crate::library::{LibraryEvent, LibraryService, ScanHandle};
use crate::playback::{AudioEngine, PlaybackController, PlaybackEvent, PlaybackStatus};
use crate::settings::Settings;
use crate::ui::{apply_theme, load_css, MainWindow};

use state::AppState;

/// Aplikasi MusikKu.
pub struct MusikKuApp {
    application: adw::Application,
    state: RefCell<Option<Rc<AppState>>>,
    window: Rc<RefCell<Option<Rc<MainWindow>>>>,
    /// Kendali pemindaian yang sedang berjalan.
    scan: Rc<RefCell<Option<ScanHandle>>>,
}

impl MusikKuApp {
    /// Buat aplikasi dan hubungkan siklus hidungnya.
    pub fn new() -> Rc<Self> {
        let application = adw::Application::builder()
            .application_id("io.github.musikku.MusikKu")
            .build();

        let app = Rc::new(Self {
            application,
            state: RefCell::new(None),
            window: Rc::new(RefCell::new(None)),
            scan: Rc::new(RefCell::new(None)),
        });

        let weak = Rc::downgrade(&app);
        app.application.connect_activate(move |_| {
            if let Some(app) = weak.upgrade() {
                app.start();
            }
        });

        app
    }

    /// Siapkan state, buka database, dan tampilkan jendela.
    fn start(self: &Rc<Self>) {
        // Jangan buat jendela kedua kali.
        if self.state.borrow().is_some() {
            if let Some(window) = self.window.borrow().as_ref() {
                window.window().present();
            }
            return;
        }

        let data_dir = self.data_dir();
        let artwork_dir = data_dir.join("artwork");
        let database_path = data_dir.join("musikku.db");

        let database = match Database::open(&database_path) {
            Ok(db) => Rc::new(db),
            Err(err) => {
                eprintln!("Gagal membuka database: {err}");
                std::process::exit(1);
            }
        };

        let settings_repo = SettingsRepository::new(database.connection());
        let settings = Settings::load(&settings_repo, artwork_dir.clone(), database_path.clone());

        let library = LibraryService::new(Rc::clone(&database), artwork_dir);
        let engine = AudioEngine::new();
        let playback = PlaybackController::new(Arc::clone(&engine));

        // Terapkan preferensi volume, acak, dan pengulangan.
        playback.set_volume(settings.default_volume);
        playback.set_shuffle(settings.shuffle);
        playback.set_repeat_mode(settings.repeat_mode);

        let state = AppState::new(
            Rc::clone(&database),
            library,
            Arc::clone(&playback),
            settings.clone(),
        );

        // Pulihkan antrean dan lagu terakhir bila diizinkan.
        if settings.restore_last_playback {
            self.restore_playback(&state, &playback);
        }

        // Simpan state playback ketika aplikasi ditutup.
        let state_for_close = Rc::clone(&state);
        self.application.connect_shutdown(move |_| {
            state_for_close.save_playback_state();
            state_for_close.save_settings();
        });

        apply_theme(settings.theme);
        load_css();

        let window = Rc::new(MainWindow::new(&self.application, Rc::clone(&state)));
        window.window().present();
        window.refresh_all();

        // Dialog memakai jendela utama sebagai induk.
        state.set_main_window(window.window().clone());

        // Perbarui state saat playback berubah.
        self.connect_playback_events(&state);

        // Terapkan perubahan halaman dan refresh tampilan.
        self.start_ui_poller(&state);

        // Mulai pemindaian bila diaktifkan di pengaturan.
        if settings.scan_on_startup {
            self.start_scan(&state);
        }

        // Integrasi MPRIS agar dapat dikendalikan dari media control desktop.
        self.start_mpris(&state);

        *self.state.borrow_mut() = Some(state);
        *self.window.borrow_mut() = Some(window);
    }

    /// Jalankan server MPRIS di thread terpisah.
    ///
    /// Runtime async harus tetap hidup selama aplikasi berjalan; kalau
    /// runtime dibuang, bus D-Bus berhenti melayani permintaan.
    ///
    /// Kegagalan D-Bus tidak menghentikan aplikasi; integrasi media
    /// control hanya dilewati bila bus tidak tersedia.
    fn start_mpris(&self, state: &Rc<AppState>) {
        let controller = Arc::clone(&state.playback);

        // Thread khusus menjalankan runtime async dan menahannya sampai
        // proses berakhir.
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(err) => {
                    log::warn!("Runtime async tidak dapat dibuat: {err}");
                    return;
                }
            };

            let server =
                runtime.block_on(async { crate::mpris::MprisServer::new(controller).await });

            let Some(_server) = server else {
                log::info!("MPRIS tidak tersedia (D-Bus tidak berjalan)");
                return;
            };

            log::info!("MPRIS aktif");

            // Tahan runtime agar bus D-Bus tetap melayani permintaan.
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        });

        // Data MPRIS diperbarui dari thread utama agar state playback
        // tidak dibaca lintas thread.
        let state_for_mpris = Rc::clone(state);
        glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            let playback = &state_for_mpris.playback;
            let track = playback.current_track();
            let snapshot = crate::mpris::MprisState {
                track,
                position_ms: playback.position_ms(),
                duration_ms: playback.duration_ms(),
                volume: playback.volume(),
                muted: playback.is_muted(),
                shuffle: playback.shuffle_enabled(),
                repeat: playback.repeat_mode(),
                playing: playback.status() == PlaybackStatus::Playing,
            };
            crate::mpris::update_shared_state(snapshot);
            glib::ControlFlow::Continue
        });
    }

    /// Periksa perubahan halaman, posisi playback, dan pesan status.
    fn start_ui_poller(self: &Rc<Self>, state: &Rc<AppState>) {
        let state = Rc::clone(state);
        let window_slot = Rc::clone(&self.window);
        let app = Rc::clone(self);

        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            while let Some(page) = state.poll_page_change() {
                if let Some(window) = window_slot.borrow().as_ref() {
                    window.apply_page(page);
                }
            }

            // Permintaan pindai ulang dari halaman pengaturan.
            if state.poll_scan_request() {
                app.start_scan(&state);
            }

            // Permintaan muat ulang daftar playlist.
            if state.poll_playlist_refresh() {
                if let Some(window) = window_slot.borrow().as_ref() {
                    window.refresh_playlists();
                }
            }

            let borrowed = window_slot.borrow();
            if let Some(window) = borrowed.as_ref() {
                window.refresh_position();
                window.refresh_player();

                // Tampilkan pesan status sebagai toast.
                if let Some(message) = state.take_status_message() {
                    window.show_status(&message);
                }
            }

            glib::ControlFlow::Continue
        });
    }

    /// Direktori data aplikasi.
    fn data_dir(&self) -> std::path::PathBuf {
        let base = std::env::var("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|home| std::path::PathBuf::from(home).join(".local").join("share"))
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
            });
        base.join("musikku")
    }

    /// Pulihkan antrean dan posisi playback dari database.
    fn restore_playback(&self, state: &Rc<AppState>, playback: &Arc<PlaybackController>) {
        let repo = PlaybackRepository::new(state.database.connection());
        let saved = match repo.load_state() {
            Ok(saved) => saved,
            Err(err) => {
                log::warn!("Gagal membaca state playback: {err}");
                return;
            }
        };

        playback.set_volume(saved.volume);
        playback.set_muted(saved.muted);
        playback.set_shuffle(saved.shuffle_enabled);
        playback.set_repeat_mode(saved.repeat_mode);

        if saved.queue_order.is_empty() {
            return;
        }

        // Bangun antrean dari track yang masih ada di library.
        let track_repo = TrackRepository::new(state.database.connection());
        let tracks = track_repo
            .find_by_ids(&saved.queue_order)
            .unwrap_or_default();
        if tracks.is_empty() {
            return;
        }

        let start = saved
            .current_track_id
            .and_then(|id| tracks.iter().position(|t| t.id == id))
            .unwrap_or(0);

        playback.play_tracks(&tracks, start);
        playback.seek(saved.position_ms);
    }

    /// Hubungkan event playback dengan pembarui UI.
    ///
    /// Callback GStreamer berjalan di thread lain, jadi per_LEARNer
    /// diteruskan lewat channel ke thread utama.
    fn connect_playback_events(&self, state: &Rc<AppState>) {
        let (tx, rx) = std::sync::mpsc::channel::<PlaybackEvent>();

        state.playback.add_listener(move |event| {
            let _ = tx.send(event);
        });

        let state_for_timer = Rc::clone(state);
        glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
            while let Ok(event) = rx.try_recv() {
                match event {
                    PlaybackEvent::TrackChanged(Some(track)) => {
                        state_for_timer.mark_played(track.id);
                    }
                    PlaybackEvent::TrackChanged(None) => {}
                    PlaybackEvent::StatusChanged(status) => {
                        if status == PlaybackStatus::Stopped {
                            state_for_timer.clear_status_message();
                        }
                    }
                    PlaybackEvent::Error(message) => {
                        log::error!("Pemutaran gagal: {message}");
                        state_for_timer.set_status_message(format!(
                            "Gagal memutar lagu: {message}. Melewati lagu berikutnya."
                        ));
                    }
                    _ => {}
                }
            }
            glib::ControlFlow::Continue
        });
    }

    /// Mulai pemindaian library di luar main thread.
    fn start_scan(&self, state: &Rc<AppState>) {
        if self.scan.borrow().is_some() {
            return;
        }

        let handle = state.library.start_scan();
        *self.scan.borrow_mut() = Some(handle);

        let state_for_scan = Rc::clone(state);
        let scan = Rc::clone(&self.scan);

        // Periksa hasil pemindaian secara berkala tanpa memblokir UI.
        glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
            let mut finished = false;
            let events = {
                let borrowed = scan.borrow();
                let Some(handle) = borrowed.as_ref() else {
                    return glib::ControlFlow::Break;
                };
                let mut collected = Vec::new();
                while let Some(event) = handle.poll() {
                    collected.push(event);
                }
                collected
            };

            for event in events {
                match event {
                    LibraryEvent::Started { total } => {
                        state_for_scan.set_status_message(format!("Memindai {total} berkas..."));
                    }
                    LibraryEvent::TrackFound(track) => {
                        state_for_scan.library.persist_track(&track);
                    }
                    LibraryEvent::Progress(progress) => {
                        state_for_scan.set_status_message(format!(
                            "Memindai {}/{}",
                            progress.processed, progress.total
                        ));
                    }
                    LibraryEvent::Finished {
                        added,
                        failed,
                        cancelled,
                    } => {
                        state_for_scan.library.mark_folders_scanned();
                        state_for_scan.refresh_library();
                        let message = if cancelled {
                            format!("Pemindaian dibatalkan. {added} lagu ditambahkan.")
                        } else if failed > 0 {
                            format!(
                                "{added} lagu ditambahkan, {failed} berkas bermasalah dilewati."
                            )
                        } else {
                            format!("{added} lagu ditambahkan.")
                        };
                        state_for_scan.set_status_message(message);
                        finished = true;
                    }
                    LibraryEvent::Failed(error) => {
                        state_for_scan.set_status_message(format!("Pemindaian gagal: {error}"));
                        finished = true;
                    }
                }
            }

            if finished {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    /// Jalankan aplikasi.
    pub fn run(self: Rc<Self>) -> glib::ExitCode {
        self.application.run()
    }
}
