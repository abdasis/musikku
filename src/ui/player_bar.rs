//! Player bar di bagian bawah window utama.

use std::rc::Rc;

use gtk::prelude::*;

use crate::app::state::AppState;
use crate::core::model::RepeatMode;
use crate::core::time::format_duration;
use crate::playback::PlaybackStatus;

use super::artwork;

/// Player bar yang selalu terlihat selama window utama aktif.
pub struct PlayerBar {
    /// Versi widget yang sudah di-upcast, untuk disimpan ke container.
    view: gtk::Widget,
    /// Tombol play/pause.
    play_button: gtk::Button,
    /// Tombol sebelumnya dan berikutnya.
    previous_button: gtk::Button,
    next_button: gtk::Button,
    /// Tombol acak dan pengulangan.
    shuffle_button: gtk::Button,
    repeat_button: gtk::Button,
    /// Tombol favorit.
    favorite_button: gtk::Button,
    /// Slider posisi dan slider volume.
    position_scale: gtk::Scale,
    volume_scale: gtk::Scale,
    /// Label judul, artis, waktu, dan durasi.
    title_label: gtk::Label,
    artist_label: gtk::Label,
    position_label: gtk::Label,
    duration_label: gtk::Label,
    /// Thumbnail cover.
    cover: gtk::Box,
    /// Panel di kiri yang disembunyikan pada window sempit.
    info_box: gtk::Box,
    /// Panel kanan yang disembunyikan pada window sempit.
    right: gtk::Box,
    state: Rc<AppState>,
    /// True ketika pengguna sedang menggeser slider posisi.
    seeking: std::cell::Cell<bool>,
}

impl PlayerBar {
    pub fn new(state: Rc<AppState>) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        widget.add_css_class("player-bar");
        widget.set_margin_start(12);
        widget.set_margin_end(12);
        widget.set_margin_top(6);
        widget.set_margin_bottom(6);

        // ---- Area kiri: cover, judul, artis, favorit ----
        let info_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);

        let cover = gtk::Box::new(gtk::Orientation::Vertical, 0);
        cover.set_size_request(48, 48);
        cover.set_halign(gtk::Align::Center);
        cover.set_valign(gtk::Align::Center);
        info_box.append(&cover);

        let labels = gtk::Box::new(gtk::Orientation::Vertical, 0);
        labels.set_valign(gtk::Align::Center);
        labels.set_hexpand(true);

        let title_label = gtk::Label::new(None);
        title_label.set_xalign(0.0);
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title_label.add_css_class("player-title");
        labels.append(&title_label);

        let artist_label = gtk::Label::new(None);
        artist_label.set_xalign(0.0);
        artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist_label.add_css_class("player-artist");
        labels.append(&artist_label);

        info_box.append(&labels);

        let favorite_button = gtk::Button::new();
        favorite_button.set_icon_name("starred-symbolic");
        favorite_button.set_tooltip_text(Some("Tandai sebagai favorit"));
        favorite_button.add_css_class("flat");
        favorite_button.set_valign(gtk::Align::Center);
        favorite_button.update_property(&[gtk::accessible::Property::Label("Favorit")]);
        info_box.append(&favorite_button);

        widget.append(&info_box);

        // ---- Area tengah: kontrol playback dan posisi ----
        let center = gtk::Box::new(gtk::Orientation::Vertical, 0);
        center.set_hexpand(true);
        center.set_halign(gtk::Align::Center);

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        buttons.set_halign(gtk::Align::Center);

        let previous_button = icon_button("media-skip-backward-symbolic", "Lagu sebelumnya");
        let play_button = icon_button("media-playback-start-symbolic", "Putar");
        play_button.add_css_class("suggested-action");
        play_button.add_css_class("play-button");
        let next_button = icon_button("media-skip-forward-symbolic", "Lagu berikutnya");

        buttons.append(&previous_button);
        buttons.append(&play_button);
        buttons.append(&next_button);
        center.append(&buttons);

        let seek_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        seek_row.set_valign(gtk::Align::Center);

        let position_label = gtk::Label::new(Some("0:00"));
        position_label.add_css_class("player-time");

        let position_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1000.0, 10.0);
        position_scale.set_hexpand(true);
        position_scale.set_draw_value(false);
        position_scale.set_valign(gtk::Align::Center);

        let duration_label = gtk::Label::new(Some("0:00"));
        duration_label.add_css_class("player-time");

        seek_row.append(&position_label);
        seek_row.append(&position_scale);
        seek_row.append(&duration_label);
        center.append(&seek_row);

        widget.append(&center);

        // ---- Area kanan: shuffle, repeat, volume ----
        let right = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        right.set_valign(gtk::Align::Center);

        let shuffle_button = icon_button("media-playlist-shuffle-symbolic", "Acak urutan");
        let repeat_button = icon_button("media-playlist-repeat-symbolic", "Ulangi");
        right.append(&shuffle_button);
        right.append(&repeat_button);

        let volume_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.05);
        volume_scale.set_draw_value(false);
        volume_scale.set_value(state.playback.volume());
        volume_scale.set_width_request(80);
        volume_scale.set_valign(gtk::Align::Center);
        volume_scale.set_tooltip_text(Some("Volume"));
        volume_scale.update_property(&[gtk::accessible::Property::Label("Volume")]);
        right.append(&volume_scale);

        widget.append(&right);

        let bar = Self {
            view: widget.clone().upcast(),
            play_button,
            previous_button,
            next_button,
            shuffle_button,
            repeat_button,
            favorite_button,
            position_scale,
            volume_scale,
            title_label,
            artist_label,
            position_label,
            duration_label,
            cover,
            info_box,
            right,
            state,
            seeking: std::cell::Cell::new(false),
        };

        bar.connect_signals();
        bar.refresh_static_state();
        bar
    }

    pub fn widget(&self) -> &gtk::Widget {
        &self.view
    }

    /// Pasang aksi klik pada tombol.
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
        self.volume_scale.connect_value_changed(move |scale| {
            state.playback.set_volume(scale.value());
        });

        // Seek hanya dikirim setelah pengguna selesai menggeser slider.
        let state = Rc::clone(&self.state);
        self.position_scale.connect_value_changed(move |scale| {
            let position = scale.value() as i64;
            if position > 0 {
                state.playback.seek(position);
            }
        });
    }

    /// Perbarui tampilan dari keadaan playback saat ini.
    pub fn refresh(&self) {
        let playback = &self.state.playback;
        let track = playback.current_track();

        match &track {
            Some(track) => {
                self.title_label.set_text(track.display_title());
                self.artist_label.set_text(track.display_artist());
                self.refresh_cover(track);
                self.favorite_button
                    .set_icon_name(if track.is_favorite {
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

        // Status tombol play/pause.
        let status = playback.status();
        let (icon, tooltip) = match status {
            PlaybackStatus::Playing => ("media-playback-pause-symbolic", "Jeda"),
            PlaybackStatus::Paused | PlaybackStatus::Buffering => {
                ("media-playback-start-symbolic", "Putar")
            }
            PlaybackStatus::Stopped => ("media-playback-start-symbolic", "Putar"),
        };
        self.play_button.set_icon_name(icon);
        self.play_button.set_tooltip_text(Some(tooltip));
        self.play_button
            .update_property(&[gtk::accessible::Property::Label(tooltip)]);

        // Mode acak dan pengulangan.
        let shuffle_on = playback.shuffle_enabled();
        set_toggled(&self.shuffle_button, shuffle_on);

        let repeat_mode = playback.repeat_mode();
        self.repeat_button.set_icon_name(repeat_mode.icon_name());
        set_toggled(&self.repeat_button, repeat_mode != RepeatMode::Off);

        // Volume dan bisu.
        self.volume_scale.set_value(playback.volume());
        let muted = playback.is_muted();
        self.volume_scale
            .set_tooltip_text(Some(if muted { "Bisukan" } else { "Volume" }));
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

    /// Ganti thumbnail cover.
    fn refresh_cover(&self, track: &crate::core::model::Track) {
        while let Some(child) = self.cover.first_child() {
            self.cover.remove(&child);
        }
        let widget = artwork::artwork_for_track(track, 48);
        self.cover.append(&widget);
    }

    /// Kembalikan cover ke placeholder.
    fn refresh_cover_placeholder(&self) {
        while let Some(child) = self.cover.first_child() {
            self.cover.remove(&child);
        }
        let widget = artwork::placeholder(48);
        self.cover.append(&widget);
    }

    /// Perbarui bagian statis seperti tooltip tombol.
    fn refresh_static_state(&self) {
        self.previous_button
            .set_tooltip_text(Some("Lagu sebelumnya"));
        self.next_button
            .set_tooltip_text(Some("Lagu berikutnya"));
    }

    /// Sembunyikan informasi sekunder pada window sempit.
    ///
    /// Tombol playback utama tetap terlihat; yang disembunyikan hanya
    /// judul lagu, artis, dan kontrol volume.
    pub fn set_compact(&self, compact: bool) {
        self.info_box.set_visible(!compact);
        self.right.set_visible(!compact);
    }
}

/// Buat tombol ikon dengan label aksesibel.
fn icon_button(icon_name: &str, label: &str) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_icon_name(icon_name);
    button.set_tooltip_text(Some(label));
    button.add_css_class("flat");
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
}

/// Tandai tombol sebagai aktif secara visual.
fn set_toggled(button: &gtk::Button, active: bool) {
    if active {
        button.add_css_class("active");
    } else {
        button.remove_css_class("active");
    }
}
