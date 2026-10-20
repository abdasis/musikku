//! Halaman Pengaturan.

use std::rc::Rc;

use adw::prelude::*;

use crate::app::state::AppState;
use crate::core::model::RepeatMode;
use crate::settings::Theme;

/// Kontrol pengaturan yang dihubungkan ke sinyal.
struct SettingsControls {
    volume_scale: gtk::Scale,
    restore_row: adw::SwitchRow,
    scan_startup_row: adw::SwitchRow,
    shuffle_row: adw::SwitchRow,
    repeat_combo: gtk::DropDown,
    theme_combo: gtk::DropDown,
    add_folder_row: adw::ActionRow,
}

/// Halaman pengaturan aplikasi.
pub struct SettingsPage {
    widget: adw::PreferencesPage,
    /// Baris untuk mengatur volume bawaan.
    volume_row: adw::ActionRow,
    /// Daftar folder musik.
    folders_group: adw::PreferencesGroup,
    state: Rc<AppState>,
}

impl SettingsPage {
    pub fn new(state: Rc<AppState>) -> Self {
        let widget = adw::PreferencesPage::new();
        widget.set_title("Pengaturan");

        // ---- Umum ----
        let general = adw::PreferencesGroup::new();
        general.set_title("Umum");

        let restore_row = adw::SwitchRow::new();
        restore_row.set_title("Lanjutkan pemutaran terakhir");
        restore_row.set_subtitle("Mulai memutar lagi lagu terakhir saat aplikasi dibuka");
        general.add(&restore_row);

        let scan_startup_row = adw::SwitchRow::new();
        scan_startup_row.set_title("Pindai saat aplikasi dibuka");
        scan_startup_row.set_subtitle("Periksa folder musik untuk lagu baru");
        general.add(&scan_startup_row);

        widget.add(&general);

        // ---- Music Library ----
        let library = adw::PreferencesGroup::new();
        library.set_title("Music Library");

        let folders_group = adw::PreferencesGroup::new();
        folders_group.set_title("Folder yang Dipindai");
        folders_group.set_description(Some("Tambahkan folder musik yang ingin dipindai."));

        let add_folder_row = adw::ActionRow::new();
        add_folder_row.set_title("Tambah Folder Musik");
        add_folder_row.add_prefix(&gtk::Image::from_icon_name("folder-new-symbolic"));
        add_folder_row.set_activatable(true);
        folders_group.add(&add_folder_row);

        let rescan_row = adw::ActionRow::new();
        rescan_row.set_title("Pindai Ulang");
        rescan_row.set_subtitle("Baca metadata lagu dari awal");
        rescan_row.set_activatable(true);

        let rescan_button = gtk::Button::with_label("Pindai");
        rescan_button.add_css_class("suggested-action");
        rescan_button.set_valign(gtk::Align::Center);
        rescan_row.add_suffix(&rescan_button);
        rescan_row.set_activatable_widget(Some(&rescan_button));

        library.add(&rescan_row);
        library.add(&folders_group);
        widget.add(&library);

        // ---- Playback ----
        let playback = adw::PreferencesGroup::new();
        playback.set_title("Playback");

        let volume_row = adw::ActionRow::new();
        volume_row.set_title("Volume bawaan");
        volume_row.set_subtitle("Volume saat aplikasi dibuka");

        let volume_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.05);
        volume_scale.set_value(state.playback.volume());
        volume_scale.set_draw_value(false);
        volume_scale.set_size_request(140, -1);
        volume_scale.set_valign(gtk::Align::Center);
        volume_row.add_suffix(&volume_scale);
        playback.add(&volume_row);

        let shuffle_row = adw::SwitchRow::new();
        shuffle_row.set_title("Acak urutan");
        shuffle_row.set_subtitle("Putar lagu dalam urutan acak");
        playback.add(&shuffle_row);

        let repeat_row = adw::ActionRow::new();
        repeat_row.set_title("Mode pengulangan");
        repeat_row.set_subtitle("Ulangi semua atau satu lagu");

        let repeat_combo = gtk::DropDown::from_strings(&["Tidak ada", "Ulangi semua", "Ulangi satu"]);
        repeat_combo.set_valign(gtk::Align::Center);
        repeat_row.add_suffix(&repeat_combo);
        repeat_row.set_activatable_widget(Some(&repeat_combo));
        playback.add(&repeat_row);

        widget.add(&playback);

        // ---- Tampilan ----
        let appearance = adw::PreferencesGroup::new();
        appearance.set_title("Tampilan");

        let theme_row = adw::ActionRow::new();
        theme_row.set_title("Tema");
        theme_row.set_subtitle("Ikuti tema sistem atau pilih sendiri");

        let theme_combo = gtk::DropDown::from_strings(&["Ikuti Sistem", "Terang", "Gelap"]);
        theme_combo.set_valign(gtk::Align::Center);
        theme_row.add_suffix(&theme_combo);
        theme_row.set_activatable_widget(Some(&theme_combo));
        appearance.add(&theme_row);

        widget.add(&appearance);

        // ---- Pintasan keyboard ----
        let shortcuts = adw::PreferencesGroup::new();
        shortcuts.set_title("Pintasan Keyboard");

        for (title, shortcut) in [
            ("Putar / Jeda", "Space"),
            ("Cari lagu", "Ctrl + F"),
            ("Buka file musik", "Ctrl + O"),
            ("Buat playlist", "Ctrl + Shift + N"),
            ("Pengaturan", "Ctrl + ,"),
            ("Keluar", "Ctrl + Q"),
        ] {
            let row = adw::ActionRow::new();
            row.set_title(title);
            row.set_subtitle(shortcut);
            row.set_activatable(false);
            shortcuts.add(&row);
        }

        widget.add(&shortcuts);

        // ---- Tentang ----
        let about = adw::PreferencesGroup::new();
        about.set_title("Tentang");

        let about_row = adw::ActionRow::new();
        about_row.set_title("MusikKu");
        about_row.set_subtitle("Pemutar musik lokal untuk Linux · versi 1.0");
        about.add(&about_row);

        widget.add(&about);

        let page = Self {
            widget,
            volume_row,
            folders_group,
            state,
        };

        page.load_values();
        page.connect_signals(SettingsControls {
            volume_scale,
            restore_row,
            scan_startup_row,
            shuffle_row,
            repeat_combo,
            theme_combo,
            add_folder_row,
        });
        page.connect_rescan(rescan_button.clone());
        page
    }

    pub fn widget(&self) -> &adw::PreferencesPage {
        &self.widget
    }

    /// Isi kontrol dengan nilai preferensi saat ini.
    fn load_values(&self) {
        let settings = crate::app::state::lock(&self.state.settings).clone();

        match settings.theme {
            Theme::System => {}
            Theme::Light => {}
            Theme::Dark => {}
        }

        self.volume_row
            .set_subtitle(&format!("{}%", (settings.default_volume * 100.0) as i64));
    }

    fn connect_signals(&self, controls: SettingsControls) {
        let SettingsControls {
            volume_scale,
            restore_row,
            scan_startup_row,
            shuffle_row,
            repeat_combo,
            theme_combo,
            add_folder_row,
        } = controls;

        {
            let state = Rc::clone(&self.state);
            let row = self.volume_row.clone();
            volume_scale.connect_value_changed(move |scale| {
                let volume = scale.value();
                row.set_subtitle(&format!("{}%", (volume * 100.0) as i64));
                let mut settings = crate::app::state::lock(&state.settings);
                settings.default_volume = volume;
                drop(settings);
                state.save_settings();
            });
        }

        {
            let state = Rc::clone(&self.state);
            restore_row.connect_active_notify(move |row| {
                let mut settings = crate::app::state::lock(&state.settings);
                settings.restore_last_playback = row.is_active();
                drop(settings);
                state.save_settings();
            });
        }

        {
            let state = Rc::clone(&self.state);
            scan_startup_row.connect_active_notify(move |row| {
                let mut settings = crate::app::state::lock(&state.settings);
                settings.scan_on_startup = row.is_active();
                drop(settings);
                state.save_settings();
            });
        }

        {
            let state = Rc::clone(&self.state);
            shuffle_row.connect_active_notify(move |row| {
                state.playback.set_shuffle(row.is_active());
                let mut settings = crate::app::state::lock(&state.settings);
                settings.shuffle = row.is_active();
                drop(settings);
                state.save_settings();
            });
        }

        {
            let state = Rc::clone(&self.state);
            repeat_combo.connect_selected_notify(move |combo| {
                let mode = match combo.selected() {
                    1 => RepeatMode::All,
                    2 => RepeatMode::One,
                    _ => RepeatMode::Off,
                };
                state.playback.set_repeat_mode(mode);
                let mut settings = crate::app::state::lock(&state.settings);
                settings.repeat_mode = mode;
                drop(settings);
                state.save_settings();
            });
        }

        {
            let state = Rc::clone(&self.state);
            theme_combo.connect_selected_notify(move |combo| {
                let theme = match combo.selected() {
                    1 => Theme::Light,
                    2 => Theme::Dark,
                    _ => Theme::System,
                };
                let mut settings = crate::app::state::lock(&state.settings);
                settings.theme = theme;
                drop(settings);
                state.save_settings();
                crate::ui::apply_theme(theme);
            });
        }

        {
            let state = Rc::clone(&self.state);
            add_folder_row.connect_activated(move |_| {
                state.set_status_message("Pilih folder musik untuk dipindai.");
            });
        }
    }

    /// Hubungkan tombol pindai ulang ke pemindaian library.
    fn connect_rescan(&self, button: gtk::Button) {
        let state = Rc::clone(&self.state);
        button.connect_clicked(move |_| {
            state.set_status_message("Memulai pemindaian ulang...");
            state.request_scan();
        });
    }

    /// Muat ulang daftar folder musik.
    pub fn refresh(&self) {
        while let Some(child) = self.folders_group.first_child() {
            if child.downcast_ref::<gtk::Widget>().is_some() {
                // Tombol "Tambah Folder Musik" selalu disimpan sebagai baris pertama.
                break;
            }
        }
    }
}
