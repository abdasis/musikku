//! Panel Now Playing beserta antrean di sisi kanan window.

use std::rc::Rc;

use gtk::gdk;
use gtk::gio;
use gtk::prelude::*;

use crate::app::state::AppState;
use crate::core::model::{RepeatMode, Track};
use crate::core::time::format_duration;
use crate::playback::PlaybackStatus;

use super::artwork;

/// Sesuaikan indeks tujuan drop dengan indeks sumber.
///
/// Bila baris sumber berada sebelum baris tujuan, indeks tujuan digeser
/// satu posisi karena baris sumber akan terangkat saat dipindahkan.
/// Mengembalikan `None` bila posisi tidak berubah.
pub fn drop_target_index(source_index: usize, target_index: usize) -> Option<usize> {
    let adjusted = if source_index < target_index {
        target_index.checked_sub(1)?
    } else {
        target_index
    };

    (adjusted != source_index).then_some(adjusted)
}

/// Tentukan indeks baris pada titik tertentu dari tinggi tiap baris.
///
/// Mengembalikan `None` bila daftar kosong. Bila titik jatuh di luar
/// baris terakhir, baris terakhir yang dipakai.
pub fn row_index_at(heights: impl Iterator<Item = f64>, y: f64) -> Option<usize> {
    let mut offset = 0.0;
    let mut count = 0;

    for (index, height) in heights.enumerate() {
        if y < offset + height {
            return Some(index);
        }
        offset += height;
        count += 1;
    }

    // Jatuh di luar baris mana pun: pakai baris terakhir.
    if count == 0 {
        return None;
    }
    Some(count - 1)
}

/// Panel Now Playing dan antrean.
pub struct NowPlayingPanel {
    widget: gtk::Box,
    /// Container cover yang diganti isinya saat lagu berganti.
    cover: gtk::Box,
    title_label: gtk::Label,
    artist_label: gtk::Label,
    favorite_button: gtk::Button,
    play_button: gtk::Button,
    previous_button: gtk::Button,
    next_button: gtk::Button,
    shuffle_button: gtk::Button,
    repeat_button: gtk::Button,
    position_scale: gtk::Scale,
    position_label: gtk::Label,
    duration_label: gtk::Label,
    /// Daftar antrean.
    queue_list: gtk::ListBox,
    queue_count_label: gtk::Label,
    clear_queue_button: gtk::Button,
    state: Rc<AppState>,
    seeking: std::cell::Cell<bool>,
}

impl NowPlayingPanel {
    pub fn new(state: Rc<AppState>) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.add_css_class("now-playing");
        widget.set_size_request(280, -1);

        // ---- Cover album ----
        let cover = gtk::Box::new(gtk::Orientation::Vertical, 0);
        cover.set_halign(gtk::Align::Center);
        cover.set_margin_top(16);
        cover.set_margin_bottom(12);
        widget.append(&cover);

        // ---- Judul dan artis ----
        let title_label = gtk::Label::new(Some("Tidak ada yang diputar"));
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title_label.add_css_class("now-playing-title");
        title_label.set_halign(gtk::Align::Center);
        widget.append(&title_label);

        let artist_label = gtk::Label::new(Some(""));
        artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist_label.add_css_class("now-playing-artist");
        artist_label.set_halign(gtk::Align::Center);
        widget.append(&artist_label);

        // ---- Tombol favorit dan opsi lain ----
        let action_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        action_row.set_halign(gtk::Align::Center);
        action_row.set_margin_top(6);

        let favorite_button = gtk::Button::new();
        favorite_button.set_icon_name("non-starred-symbolic");
        favorite_button.set_tooltip_text(Some("Tandai sebagai favorit"));
        favorite_button.add_css_class("flat");
        favorite_button.update_property(&[gtk::accessible::Property::Label("Favorit")]);

        let more_button = gtk::MenuButton::new();
        more_button.set_icon_name("view-more-symbolic");
        more_button.set_tooltip_text(Some("Opsi lain"));
        more_button.add_css_class("flat");
        more_button.update_property(&[gtk::accessible::Property::Label("Opsi lain")]);

        action_row.append(&favorite_button);
        action_row.append(&more_button);
        widget.append(&action_row);

        // ---- Seek bar ----
        let seek_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        seek_row.set_margin_top(12);
        seek_row.set_margin_start(16);
        seek_row.set_margin_end(16);

        let position_label = gtk::Label::new(Some("0:00"));
        position_label.add_css_class("player-time");

        let position_scale =
            gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1000.0, 10.0);
        position_scale.set_hexpand(true);
        position_scale.set_draw_value(false);

        let duration_label = gtk::Label::new(Some("0:00"));
        duration_label.add_css_class("player-time");

        seek_row.append(&position_label);
        seek_row.append(&position_scale);
        seek_row.append(&duration_label);
        widget.append(&seek_row);

        // ---- Kontrol pemutaran ----
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.set_halign(gtk::Align::Center);
        controls.set_margin_top(8);

        let shuffle_button = gtk::Button::new();
        shuffle_button.set_icon_name("media-playlist-shuffle-symbolic");
        shuffle_button.set_tooltip_text(Some("Acak urutan"));
        shuffle_button.add_css_class("flat");
        shuffle_button.update_property(&[gtk::accessible::Property::Label("Acak urutan")]);

        let previous_button = gtk::Button::new();
        previous_button.set_icon_name("media-skip-backward-symbolic");
        previous_button.set_tooltip_text(Some("Lagu sebelumnya"));
        previous_button.add_css_class("flat");
        previous_button.update_property(&[gtk::accessible::Property::Label("Lagu sebelumnya")]);

        let play_button = gtk::Button::new();
        play_button.set_icon_name("media-playback-start-symbolic");
        play_button.set_tooltip_text(Some("Putar"));
        play_button.add_css_class("suggested-action");
        play_button.add_css_class("play-button");
        play_button.update_property(&[gtk::accessible::Property::Label("Putar")]);

        let next_button = gtk::Button::new();
        next_button.set_icon_name("media-skip-forward-symbolic");
        next_button.set_tooltip_text(Some("Lagu berikutnya"));
        next_button.add_css_class("flat");
        next_button.update_property(&[gtk::accessible::Property::Label("Lagu berikutnya")]);

        let repeat_button = gtk::Button::new();
        repeat_button.set_icon_name("media-playlist-repeat-symbolic");
        repeat_button.set_tooltip_text(Some("Ulangi"));
        repeat_button.add_css_class("flat");
        repeat_button.update_property(&[gtk::accessible::Property::Label("Ulangi")]);

        controls.append(&shuffle_button);
        controls.append(&previous_button);
        controls.append(&play_button);
        controls.append(&next_button);
        controls.append(&repeat_button);
        widget.append(&controls);

        // ---- Antrean ----
        let queue_header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        queue_header.set_margin_top(16);
        queue_header.set_margin_start(16);
        queue_header.set_margin_end(16);

        let queue_count_label = gtk::Label::new(Some("Antrean"));
        queue_count_label.add_css_class("heading");
        queue_count_label.set_hexpand(true);
        queue_header.append(&queue_count_label);

        let clear_queue_button = gtk::Button::new();
        clear_queue_button.set_label("Bersihkan");
        clear_queue_button.add_css_class("flat");
        clear_queue_button.set_tooltip_text(Some("Kosongkan antrean"));
        clear_queue_button
            .update_property(&[gtk::accessible::Property::Label("Kosongkan antrean")]);
        queue_header.append(&clear_queue_button);

        widget.append(&queue_header);

        let queue_list = gtk::ListBox::new();
        queue_list.add_css_class("queue-list");
        queue_list.set_selection_mode(gtk::SelectionMode::Single);

        let queue_scroller = gtk::ScrolledWindow::builder()
            .child(&queue_list)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .margin_start(8)
            .margin_end(8)
            .margin_bottom(8)
            .build();
        widget.append(&queue_scroller);

        // Menu "opsi lain" untuk lagu yang sedang diputar.
        {
            let menu = gio::Menu::new();
            let section = gio::Menu::new();
            section.append(Some("Tambahkan ke Antrean"), Some("now.enqueue"));
            section.append(Some("Tampilkan di Library"), Some("now.show-library"));
            menu.append_section(None, &section);
            more_button.set_menu_model(Some(&menu));

            // Aksi menu memakai grup aksi agar mudah ditangani.
            let group = gio::SimpleActionGroup::new();

            let state_for_queue = Rc::clone(&state);
            let action = gio::SimpleAction::new("now.enqueue", None);
            action.connect_activate(move |_, _| {
                if let Some(track) = state_for_queue.playback.current_track() {
                    state_for_queue.playback.enqueue(&track);
                    state_for_queue.set_status_message(format!(
                        "{} ditambahkan ke antrean.",
                        track.display_title()
                    ));
                }
            });
            group.add_action(&action);

            let state_for_library = Rc::clone(&state);
            let action = gio::SimpleAction::new("now.show-library", None);
            action.connect_activate(move |_, _| {
                state_for_library.navigate_to(crate::app::state::Page::Library);
            });
            group.add_action(&action);

            more_button.insert_action_group("now", Some(&group));
        }

        let panel = Self {
            widget,
            cover,
            title_label,
            artist_label,
            favorite_button,
            play_button,
            previous_button,
            next_button,
            shuffle_button,
            repeat_button,
            position_scale,
            position_label,
            duration_label,
            queue_list,
            queue_count_label,
            clear_queue_button,
            state,
            seeking: std::cell::Cell::new(false),
        };

        panel.connect_signals();
        panel.connect_queue_drop();
        panel
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.widget
    }

    fn connect_signals(&self) {
        let state = Rc::clone(&self.state);

        self.play_button.connect_clicked(move |_| {
            state.playback.toggle_play_pause();
        });

        let state = Rc::clone(&self.state);
        self.previous_button.connect_clicked(move |_| {
            state.playback.previous();
        });

        let state = Rc::clone(&self.state);
        self.next_button.connect_clicked(move |_| {
            state.playback.next();
        });

        let state = Rc::clone(&self.state);
        self.shuffle_button.connect_clicked(move |_| {
            let enabled = !state.playback.shuffle_enabled();
            state.playback.set_shuffle(enabled);
        });

        let state = Rc::clone(&self.state);
        self.repeat_button.connect_clicked(move |_| {
            let next = match state.playback.repeat_mode() {
                RepeatMode::Off => RepeatMode::All,
                RepeatMode::All => RepeatMode::One,
                RepeatMode::One => RepeatMode::Off,
            };
            state.playback.set_repeat_mode(next);
        });

        let state = Rc::clone(&self.state);
        self.favorite_button.connect_clicked(move |_| {
            let Some(track) = state.playback.current_track() else {
                return;
            };
            state.set_favorite(track.id, !track.is_favorite);
        });

        let state = Rc::clone(&self.state);
        self.clear_queue_button.connect_clicked(move |_| {
            state.playback.clear_queue();
        });

        let state = Rc::clone(&self.state);
        self.position_scale.connect_value_changed(move |scale| {
            let position = scale.value() as i64;
            if position > 0 {
                state.playback.seek(position);
            }
        });
    }

    /// Perbarui panel dari keadaan playback saat ini.
    pub fn refresh(&self) {
        let playback = &self.state.playback;
        let track = playback.current_track();

        match &track {
            Some(track) => {
                self.title_label.set_text(track.display_title());
                self.artist_label.set_text(track.display_artist());
                self.refresh_cover(track);
                self.favorite_button.set_icon_name(if track.is_favorite {
                    "starred-symbolic"
                } else {
                    "non-starred-symbolic"
                });
                self.favorite_button.set_sensitive(true);
            }
            None => {
                self.title_label.set_text("Tidak ada yang diputar");
                self.artist_label.set_text("");
                self.refresh_cover_placeholder();
                self.favorite_button.set_sensitive(false);
            }
        }

        let status = playback.status();
        let (icon, tooltip) = match status {
            PlaybackStatus::Playing => ("media-playback-pause-symbolic", "Jeda"),
            _ => ("media-playback-start-symbolic", "Putar"),
        };
        self.play_button.set_icon_name(icon);
        self.play_button.set_tooltip_text(Some(tooltip));
        self.play_button
            .update_property(&[gtk::accessible::Property::Label(tooltip)]);

        let shuffle_on = playback.shuffle_enabled();
        set_toggled(&self.shuffle_button, shuffle_on);

        let repeat_mode = playback.repeat_mode();
        self.repeat_button.set_icon_name(repeat_mode.icon_name());
        set_toggled(&self.repeat_button, repeat_mode != RepeatMode::Off);
    }

    /// Perbarui posisi dan durasi.
    pub fn refresh_position(&self) {
        let position = self.state.playback.position_ms();
        let duration = self.state.playback.duration_ms();

        self.position_label.set_text(&format_duration(position));
        self.duration_label.set_text(&format_duration(duration));

        if !self.seeking.get() {
            self.position_scale.set_range(0.0, duration.max(1) as f64);
            self.position_scale.set_value(position as f64);
        }
    }

    /// Tandai bahwa pengguna sedang menggeser slider posisi.
    pub fn set_seeking(&self, seeking: bool) {
        self.seeking.set(seeking);
    }

    /// Hubungkan penanganan seretan pada daftar antrean.
    fn connect_queue_drop(&self) {
        let state = Rc::clone(&self.state);
        let queue_list = self.queue_list.clone();

        let drop_target = gtk::DropTarget::new(glib::Type::I64, gdk::DragAction::MOVE);
        drop_target.connect_drop(move |_target, value, _x, y| {
            log::debug!("Drop diterima: y={y}");
            let Some(source_id) = value.get::<i64>().ok() else {
                return false;
            };

            // Tentukan posisi tujuan dari titik jatuh di dalam daftar.
            let Some(target_index) = Self::row_at_position(&queue_list, y) else {
                return false;
            };

            let queue = state.playback.queue();
            let order = queue.order().to_vec();

            let Some(source_index) = order.iter().position(|id| *id == source_id) else {
                return false;
            };

            // Posisi tujuan disesuaikan: bila baris sumber diletakkan
            // sebelum baris tujuan, indeksnya digeser satu agar hasil
            // akhirnya sesuai pilihan pengguna.
            let Some(target_index) = drop_target_index(source_index, target_index) else {
                return true;
            };
            if target_index == source_index {
                return true;
            }

            state.playback.reorder_queue(source_index, target_index);
            state.set_status_message("Urutan antrean diubah.");
            true
        });

        self.queue_list.add_controller(drop_target);
    }

    /// Cari indeks baris daftar yang berada di titik tertentu.
    ///
    /// Koordinat drop bersifat relatif terhadap daftar, sehingga posisi
    /// tiap baris dihitung dari tinggi baris sebelumnya.
    fn row_at_position(list: &gtk::ListBox, y: f64) -> Option<usize> {
        let mut heights = Vec::new();
        let mut child = list.first_child();
        while let Some(row) = child {
            heights.push(row.height() as f64);
            child = row.next_sibling();
        }

        row_index_at(heights.into_iter(), y)
    }

    /// Bangun ulang daftar antrean.
    pub fn refresh_queue(&self) {
        while let Some(child) = self.queue_list.first_child() {
            self.queue_list.remove(&child);
        }

        let queue = self.state.playback.queue();
        let current_index = queue.current_index();
        let tracks = queue.order().to_vec();

        self.queue_count_label
            .set_text(&format!("Antrean ({})", tracks.len()));

        if tracks.is_empty() {
            let empty = gtk::Label::new(Some("Antrean kosong"));
            empty.add_css_class("dim-label");
            empty.set_margin_top(12);
            empty.set_margin_bottom(12);
            self.queue_list.append(&empty);
            return;
        }

        let snapshot = self.state.snapshot();
        for (index, track_id) in tracks.iter().enumerate() {
            let Some(track) = snapshot.track(*track_id) else {
                continue;
            };

            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);

            let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row_box.set_margin_top(4);
            row_box.set_margin_bottom(4);
            row_box.set_margin_start(4);
            row_box.set_margin_end(4);

            let labels = gtk::Box::new(gtk::Orientation::Vertical, 0);
            labels.set_hexpand(true);

            let title = gtk::Label::new(Some(track.display_title()));
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);

            let artist = gtk::Label::new(Some(track.display_artist()));
            artist.set_xalign(0.0);
            artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
            artist.add_css_class("dim-label");
            artist.add_css_class("caption");

            labels.append(&title);
            labels.append(&artist);
            row_box.append(&labels);

            let duration = gtk::Label::new(Some(&format_duration(track.duration_ms)));
            duration.add_css_class("dim-label");
            duration.add_css_class("caption");
            row_box.append(&duration);

            // Tombol pemindah urutan, sebagai alternatif seretan yang
            // juga dapat dipakai keyboard.
            let move_up = gtk::Button::from_icon_name("go-up-symbolic");
            move_up.set_tooltip_text(Some("Naikkan dalam antrean"));
            move_up.add_css_class("flat");
            move_up.set_sensitive(index > 0);
            move_up.update_property(&[gtk::accessible::Property::Label("Naikkan dalam antrean")]);
            row_box.append(&move_up);

            let move_down = gtk::Button::from_icon_name("go-down-symbolic");
            move_down.set_tooltip_text(Some("Turunkan dalam antrean"));
            move_down.add_css_class("flat");
            move_down.set_sensitive(index + 1 < tracks.len());
            move_down
                .update_property(&[gtk::accessible::Property::Label("Turunkan dalam antrean")]);
            row_box.append(&move_down);

            row.set_child(Some(&row_box));

            if current_index == Some(index) {
                row.add_css_class("queue-active");
            }

            // Pemegang baris untuk menyeret antrean. Seret dilakukan
            // dari sini, bukan dari area teks, karena area teks dipakai
            // untuk memilih dan memutar lagu.
            let handle = gtk::Image::from_icon_name("view-drag-handle-symbolic");
            handle.set_tooltip_text(Some("Seret untuk mengubah urutan"));
            handle.set_valign(gtk::Align::Center);
            handle.update_property(&[gtk::accessible::Property::Label(
                "Seret untuk mengubah urutan",
            )]);

            let drag = gtk::DragSource::new();
            drag.set_actions(gdk::DragAction::MOVE);

            let track_id = track.id;
            drag.connect_prepare(move |_, _x, _y| {
                log::debug!("Seretan disiapkan: track {track_id}");
                Some(gdk::ContentProvider::for_value(&track_id.to_value()))
            });

            handle.add_controller(drag);
            row_box.append(&handle);

            let state = Rc::clone(&self.state);
            let track_for_click = track.clone();
            row.connect_activate(move |_| {
                state.playback.play_single(&track_for_click);
            });

            {
                let state = Rc::clone(&self.state);
                move_up.connect_clicked(move |_| {
                    if index > 0 {
                        state.playback.reorder_queue(index, index - 1);
                        state.set_status_message("Lagu dipindahkan ke atas.");
                    }
                });
            }

            {
                let state = Rc::clone(&self.state);
                let track_count = tracks.len();
                move_down.connect_clicked(move |_| {
                    if index + 1 < track_count {
                        state.playback.reorder_queue(index, index + 1);
                        state.set_status_message("Lagu dipindahkan ke bawah.");
                    }
                });
            }

            self.queue_list.append(&row);
        }
    }

    /// Ganti cover album.
    fn refresh_cover(&self, track: &Track) {
        while let Some(child) = self.cover.first_child() {
            self.cover.remove(&child);
        }
        let widget = artwork::artwork_for_track(track, 220);
        self.cover.append(&widget);
    }

    /// Kembalikan cover ke placeholder.
    fn refresh_cover_placeholder(&self) {
        while let Some(child) = self.cover.first_child() {
            self.cover.remove(&child);
        }
        let widget = artwork::placeholder(220);
        self.cover.append(&widget);
    }
}

/// Tandai tombol sebagai aktif secara visual.
fn set_toggled(button: &gtk::Button, active: bool) {
    if active {
        button.add_css_class("active");
    } else {
        button.remove_css_class("active");
    }
}
