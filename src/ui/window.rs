//! Jendela utama: sidebar, area konten, panel Now Playing, dan player bar.

use std::rc::Rc;

use adw::prelude::*;

use crate::app::state::{AppState, Page};

use super::now_playing::NowPlayingPanel;
use super::pages::collection::CollectionPage;
use super::pages::home::HomePage;
use super::pages::library::LibraryPage;
use super::pages::playlists::PlaylistsPage;
use super::pages::settings::SettingsPage;
use super::player_bar::PlayerBar;
use super::sidebar::Sidebar;

/// Jendela utama aplikasi.
pub struct MainWindow {
    window: adw::ApplicationWindow,
    state: Rc<AppState>,
    sidebar: Sidebar,
    home_page: HomePage,
    library_page: LibraryPage,
    playlists_page: PlaylistsPage,
    favorites_page: CollectionPage,
    recently_page: CollectionPage,
    now_playing: NowPlayingPanel,
    player_bar: Rc<PlayerBar>,
    /// Area konten yang menampilkan halaman aktif.
    content_stack: adw::ViewStack,
    /// Panel kanan Now Playing, disembunyikan pada window sempit.
    now_playing_panel: gtk::Box,
    /// Pemisah sidebar dan konten, dipakai untuk mengukur lebar.
    split_view: adw::NavigationSplitView,
    /// Pembatas yang memisahkan konten dan panel kanan.
    panel_split: adw::NavigationSplitView,
    /// Tombol toggle panel antrean.
    queue_toggle: gtk::ToggleButton,
    /// Lapisan untuk menampilkan pesan status.
    toast_overlay: adw::ToastOverlay,
}

impl MainWindow {
    pub fn new(app: &impl IsA<gtk::Application>, state: Rc<AppState>) -> Self {
        let window = adw::ApplicationWindow::new(app);
        window.set_title(Some("MusikKu"));
        window.set_default_size(1280, 800);
        window.set_size_request(360, 480);

        // ---- Header bar ----
        let header = adw::HeaderBar::new();

        let back_button = gtk::Button::from_icon_name("go-previous-symbolic");
        back_button.set_tooltip_text(Some("Kembali"));
        back_button.add_css_class("flat");
        back_button.update_property(&[gtk::accessible::Property::Label("Kembali")]);
        header.pack_start(&back_button);

        let forward_button = gtk::Button::from_icon_name("go-next-symbolic");
        forward_button.set_tooltip_text(Some("Maju"));
        forward_button.add_css_class("flat");
        forward_button.update_property(&[gtk::accessible::Property::Label("Maju")]);
        header.pack_start(&forward_button);

        let queue_toggle = gtk::ToggleButton::new();
        queue_toggle.set_icon_name("view-list-symbolic");
        queue_toggle.set_tooltip_text(Some("Tampilkan antrean"));
        queue_toggle.add_css_class("flat");
        queue_toggle.update_property(&[gtk::accessible::Property::Label("Tampilkan antrean")]);
        header.pack_end(&queue_toggle);

        let menu_button = gtk::MenuButton::new();
        menu_button.set_icon_name("open-menu-symbolic");
        menu_button.set_tooltip_text(Some("Menu utama"));
        menu_button.update_property(&[gtk::accessible::Property::Label("Menu utama")]);
        header.pack_end(&menu_button);

        // ---- Halaman-halaman ----
        let home_page = HomePage::new(Rc::clone(&state));
        let library_page = LibraryPage::new(Rc::clone(&state));
        let playlists_page = PlaylistsPage::new(Rc::clone(&state));
        let favorites_page = CollectionPage::new_favorites(Rc::clone(&state));
        let recently_page = CollectionPage::new_recently_played(Rc::clone(&state));
        let settings_page = SettingsPage::new(Rc::clone(&state));

        let content_stack = adw::ViewStack::new();
        content_stack.add_titled(home_page.widget(), Some("home"), "Home");
        content_stack.add_titled(library_page.widget(), Some("library"), "Library");
        content_stack.add_titled(playlists_page.widget(), Some("playlists"), "Playlist");
        content_stack.add_titled(favorites_page.widget(), Some("favorites"), "Favorit");
        content_stack.add_titled(recently_page.widget(), Some("recently"), "Terakhir Diputar");
        content_stack.add_titled(settings_page.widget(), Some("settings"), "Pengaturan");

        // ---- Panel kanan Now Playing ----
        let now_playing = NowPlayingPanel::new(Rc::clone(&state));
        let now_playing_panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        now_playing_panel.set_size_request(280, -1);
        now_playing_panel.add_css_class("now-playing-panel");
        now_playing_panel.append(now_playing.widget());

        // Panel Now Playing dipisah dengan split view kedua, supaya
        // lebarnya dijaga oleh libadwaita dan panel bisa disembunyikan
        // pada window sempit.
        let panel_split = adw::NavigationSplitView::new();

        // Split view memerlukan NavigationPage untuk kedua sisinya.
        let content_child = adw::NavigationPage::new(&content_stack, "Konten");
        content_child.set_tag(Some("content"));
        let panel_child = adw::NavigationPage::new(&now_playing_panel, "Sekarang Diputar");
        panel_child.set_tag(Some("now-playing"));

        panel_split.set_content(Some(&content_child));
        panel_split.set_sidebar(Some(&panel_child));
        panel_split.set_sidebar_position(gtk::PackType::End);
        panel_split.set_collapsed(true);
        // Lebar panel dibatasi agar tidak memakan seluruh ruang.
        panel_split.set_sidebar_width_fraction(0.28);
        panel_split.set_max_sidebar_width(320.0);
        panel_split.set_min_sidebar_width(280.0);

        // ---- Sidebar ----
        let sidebar = Sidebar::new(Rc::clone(&state));

        let split_view = adw::NavigationSplitView::new();
        let paned_page = adw::NavigationPage::new(&panel_split, "Konten");
        paned_page.set_tag(Some("content"));

        split_view.set_sidebar(Some(sidebar.widget()));
        split_view.set_content(Some(&paned_page));
        split_view.set_collapsed(false);

        // ---- Player bar ----
        let player_bar = Rc::new(PlayerBar::new(Rc::clone(&state)));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&split_view);
        root.append(player_bar.widget());

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&root));

        // Lapisan toast menumpuk di atas seluruh isi jendela.
        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&toolbar));
        window.set_content(Some(&toast_overlay));

        let window_ref = Self {
            window,
            state: Rc::clone(&state),
            sidebar,
            home_page,
            library_page,
            playlists_page,
            favorites_page,
            recently_page,
            now_playing,
            player_bar: Rc::clone(&player_bar),
            content_stack,
            now_playing_panel,
            split_view,
            panel_split,
            queue_toggle,
            toast_overlay,
        };

        window_ref.connect_signals(back_button, forward_button, menu_button);
        window_ref.connect_resize();
        window_ref.apply_page(state.page());
        window_ref
    }

    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    /// Minta semua halaman memuat ulang datanya.
    pub fn refresh_all(&self) {
        self.state.refresh_library();
        self.sidebar
            .set_folder_count(self.state.library.folders().len());
        self.home_page.refresh();
        self.library_page.refresh();
        self.playlists_page.refresh();
        self.favorites_page.refresh();
        self.recently_page.refresh();
        self.now_playing.refresh_queue();
        self.now_playing.refresh();
        self.player_bar.refresh();
    }

    /// Muat ulang daftar playlist.
    pub fn refresh_playlists(&self) {
        self.playlists_page.refresh();
    }

    /// Tampilkan halaman sesuai keadaan aplikasi.
    pub fn apply_page(&self, page: Page) {
        let name = match page {
            Page::Home => "home",
            Page::Library => "library",
            Page::Playlists => "playlists",
            Page::Favorites => "favorites",
            Page::RecentlyPlayed => "recently",
            Page::Settings => "settings",
        };

        self.content_stack.set_visible_child_name(name);
        self.sidebar.set_active_page(page);

        match page {
            Page::Home => self.home_page.refresh(),
            Page::Library => self.library_page.refresh(),
            Page::Playlists => self.playlists_page.refresh(),
            Page::Favorites => self.favorites_page.refresh(),
            Page::RecentlyPlayed => self.recently_page.refresh(),
            Page::Settings => {}
        }
    }

    /// Perbarui tampilan pemutar.
    pub fn refresh_player(&self) {
        self.now_playing.refresh();
        self.player_bar.refresh();
        self.now_playing.refresh_queue();
    }

    /// Perbarui posisi playback.
    pub fn refresh_position(&self) {
        self.now_playing.refresh_position();
        self.player_bar.refresh_position();
    }

    /// Tampilkan pesan status singkat sebagai toast.
    pub fn show_status(&self, message: &str) {
        if message.trim().is_empty() {
            return;
        }
        let toast = adw::Toast::new(message);
        toast.set_timeout(4);
        self.toast_overlay.add_toast(toast);
    }

    /// Sembunyikan panel Now Playing (dipakai pada window sempit).
    pub fn set_now_playing_visible(&self, visible: bool) {
        self.now_playing_panel.set_visible(visible);
        self.queue_toggle.set_active(visible);
    }

    fn connect_signals(
        &self,
        back_button: gtk::Button,
        forward_button: gtk::Button,
        menu_button: gtk::MenuButton,
    ) {
        let state = Rc::clone(&self.state);

        back_button.connect_clicked(move |_| {
            // Kembali ke Home dari halaman lain.
            if state.page() != Page::Home {
                state.navigate_to(Page::Home);
            }
        });

        let state = Rc::clone(&self.state);
        forward_button.connect_clicked(move |_| {
            let next = match state.page() {
                Page::Home => Page::Library,
                Page::Library => Page::Playlists,
                Page::Playlists => Page::Favorites,
                Page::Favorites => Page::RecentlyPlayed,
                Page::RecentlyPlayed => Page::Settings,
                Page::Settings => Page::Home,
            };
            state.navigate_to(next);
        });

        // Menu utama.
        let menu = gtk::Popover::new();
        let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        menu_box.set_margin_top(6);
        menu_box.set_margin_bottom(6);
        menu_box.set_margin_start(6);
        menu_box.set_margin_end(6);

        let add_folder = gtk::Button::new();
        add_folder.set_label("Tambah Folder Musik");
        let state_for_folder = Rc::clone(&self.state);
        let window_for_folder = self.window.clone();
        add_folder.connect_clicked(move |_| {
            state_for_folder.navigate_to(Page::Library);
            crate::ui::folder_dialog::choose_music_folder(
                &window_for_folder,
                Rc::clone(&state_for_folder),
                |_path| {},
            );
        });
        menu_box.append(&add_folder);

        let preferences = gtk::Button::new();
        preferences.set_label("Pengaturan");
        preferences.connect_clicked({
            let state = Rc::clone(&self.state);
            move |_| {
                state.navigate_to(Page::Settings);
            }
        });
        menu_box.append(&preferences);

        menu.set_child(Some(&menu_box));
        menu_button.set_popover(Some(&menu));

        // Panel antrean dapat disembunyikan lewat collapse split view.
        let panel_split = self.panel_split.clone();
        self.queue_toggle.connect_toggled(move |toggle| {
            panel_split.set_collapsed(!toggle.is_active());
        });

        self.install_shortcuts();
    }

    /// Pasang pintasan keyboard sesuai konvensi GTK.
    ///
    /// Aksibaikan sebagai `win.*` agar tidak bentrok dengan shortcut global sistem.
    fn install_shortcuts(&self) {
        // Space: play / pause.
        self.register_action(
            "play-pause",
            gtk::gdk::Key::space,
            gtk::gdk::ModifierType::empty(),
            {
                let state = Rc::clone(&self.state);
                move |_| state.playback.toggle_play_pause()
            },
        );

        // Ctrl+F: fokus pencarian.
        self.register_action(
            "focus-search",
            gtk::gdk::Key::f,
            gtk::gdk::ModifierType::CONTROL_MASK,
            |_| {},
        );

        // Ctrl+Right / Ctrl+Left: lagu berikutnya / sebelumnya.
        self.register_action(
            "next-track",
            gtk::gdk::Key::Right,
            gtk::gdk::ModifierType::CONTROL_MASK,
            {
                let state = Rc::clone(&self.state);
                move |_| state.playback.next()
            },
        );
        self.register_action(
            "previous-track",
            gtk::gdk::Key::Left,
            gtk::gdk::ModifierType::CONTROL_MASK,
            {
                let state = Rc::clone(&self.state);
                move |_| state.playback.previous()
            },
        );

        // Alt+Left / Alt+Right: navigasi halaman.
        self.register_action(
            "go-back",
            gtk::gdk::Key::Left,
            gtk::gdk::ModifierType::ALT_MASK,
            {
                let state = Rc::clone(&self.state);
                move |_| {
                    if state.page() != Page::Home {
                        state.navigate_to(Page::Home);
                    }
                }
            },
        );
        self.register_action(
            "go-forward",
            gtk::gdk::Key::Right,
            gtk::gdk::ModifierType::ALT_MASK,
            {
                let state = Rc::clone(&self.state);
                move |_| {
                    let next = match state.page() {
                        Page::Home => Page::Library,
                        Page::Library => Page::Playlists,
                        Page::Playlists => Page::Favorites,
                        Page::Favorites => Page::RecentlyPlayed,
                        Page::RecentlyPlayed => Page::Settings,
                        Page::Settings => Page::Home,
                    };
                    state.navigate_to(next);
                }
            },
        );

        // Ctrl+Shift+N: buat playlist.
        self.register_action(
            "new-playlist",
            gtk::gdk::Key::n,
            gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK,
            {
                let state = Rc::clone(&self.state);
                move |_| {
                    state.navigate_to(Page::Playlists);
                    state.set_status_message("Buat playlist baru untuk mengorganisasi lagumu.");
                }
            },
        );

        // Ctrl+O: buka file musik.
        {
            let state = Rc::clone(&self.state);
            let window = self.window.clone();
            self.register_action(
                "open-file",
                gtk::gdk::Key::o,
                gtk::gdk::ModifierType::CONTROL_MASK,
                move |_| {
                    let state = Rc::clone(&state);
                    crate::ui::folder_dialog::choose_audio_files(&window, move |paths| {
                        state.set_status_message(format!("{} berkas dipilih.", paths.len()));
                    });
                },
            );
        }

        // Ctrl+comma: pengaturan.
        self.register_action(
            "preferences",
            gtk::gdk::Key::comma,
            gtk::gdk::ModifierType::CONTROL_MASK,
            {
                let state = Rc::clone(&self.state);
                move |_| state.navigate_to(Page::Settings)
            },
        );

        // Ctrl+Q: keluar.
        {
            let window = self.window.clone();
            self.register_action(
                "quit",
                gtk::gdk::Key::q,
                gtk::gdk::ModifierType::CONTROL_MASK,
                move |_| window.close(),
            );
        }
    }

    /// Sesuaikan tata letak saat ukuran jendela berubah.
    ///
    /// Pada window sempit panel Now Playing disembunyikan; pada window lebar
    /// panel ditampilkan kembali.
    fn connect_resize(&self) {
        let panel_split = self.panel_split.clone();
        let queue_toggle = self.queue_toggle.clone();
        let last_width = std::cell::Cell::new(0i32);
        let player_bar = Rc::clone(&self.player_bar);
        let split_view = self.split_view.clone();
        let sidebar = self.sidebar.widget().clone();

        glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
            // Lebar area konten dipakai sebagai patokan. Split view
            // memuat sidebar, jadi lebar kontennya adalah lebar split
            // view dikurangi lebar sidebar yang tampil.
            let total = split_view.width().saturating_sub(sidebar.width().max(0));
            if total <= 0 {
                // Belum dialokasikan; tunggu alokasi berikutnya.
                return glib::ControlFlow::Continue;
            }
            if total == last_width.get() {
                return glib::ControlFlow::Continue;
            }
            last_width.set(total);

            // Panel Now Playing membutuhkan 320 piksel; sisanya dipakai
            // konten. Layout tiga kolom baru muat bila tersedia.
            let content_width = panel_split.width();
            let narrow = content_width < 320 + 480;

            // Panel disembunyikan pada window sempit lewat collapse.
            panel_split.set_collapsed(narrow);
            queue_toggle.set_active(!narrow);

            // Pada window sempit, sidebar menjadi navigasi ringkas dan
            // informasi sekunder pada player bar disembunyikan.
            player_bar.set_compact(narrow);

            glib::ControlFlow::Continue
        });
    }

    /// Daftarkan aksi `win.<name>` beserta pintasan keyboardnya.
    fn register_action(
        &self,
        name: &str,
        key: gtk::gdk::Key,
        modifiers: gtk::gdk::ModifierType,
        handler: impl Fn(()) + 'static,
    ) {
        let action_name = format!("win.{name}");
        let trigger = gtk::KeyvalTrigger::new(key, modifiers);

        let controller = gtk::ShortcutController::new();
        controller.add_shortcut(gtk::Shortcut::new(
            Some(trigger),
            Some(gtk::NamedAction::new(&action_name)),
        ));
        self.window.add_controller(controller);

        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(move |_, _| handler(()));
        self.window.add_action(&action);
    }
}
