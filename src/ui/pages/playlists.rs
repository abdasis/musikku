//! Halaman Playlist: daftar playlist dan detail playlist yang dipilih.

use std::rc::Rc;

use adw::prelude::*;

use crate::app::state::AppState;
use crate::core::model::Track;
use crate::core::time::format_duration;
use crate::database::PlaylistRepository;

use super::super::artwork;

/// Halaman playlist.
pub struct PlaylistsPage {
    /// Versi widget yang sudah di-upcast.
    content: gtk::Widget,
    /// Daftar playlist di sisi kiri.
    list: gtk::ListBox,
    /// Detail playlist yang dipilih.
    detail_title: gtk::Label,
    detail_subtitle: gtk::Label,
    play_button: gtk::Button,
    shuffle_button: gtk::Button,
    /// Daftar lagu dalam playlist.
    track_list: gtk::ListBox,
    state: Rc<AppState>,
}

impl PlaylistsPage {
    pub fn new(state: Rc<AppState>) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        widget.add_css_class("playlists-page");

        // ---- Daftar playlist ----
        let list_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        list_box.set_size_request(260, -1);
        list_box.set_margin_start(12);
        list_box.set_margin_top(12);
        list_box.set_margin_bottom(12);

        let list_header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let list_title = gtk::Label::new(Some("Playlist"));
        list_title.add_css_class("heading");
        list_title.set_hexpand(true);
        list_header.append(&list_title);

        let new_button = gtk::Button::from_icon_name("list-add-symbolic");
        new_button.set_tooltip_text(Some("Buat playlist baru"));
        new_button.add_css_class("flat");
        new_button.update_property(&[gtk::accessible::Property::Label("Buat playlist baru")]);
        list_header.append(&new_button);
        list_box.append(&list_header);

        let list = gtk::ListBox::new();
        list.add_css_class("navigation-sidebar");
        list.set_selection_mode(gtk::SelectionMode::Single);

        let list_scroller = gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        list_box.append(&list_scroller);

        widget.append(&list_box);

        // ---- Detail playlist ----
        let detail_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        detail_box.set_hexpand(true);
        detail_box.set_margin_start(18);
        detail_box.set_margin_end(18);
        detail_box.set_margin_top(18);
        detail_box.set_margin_bottom(18);

        let detail_title = gtk::Label::new(Some("Pilih sebuah playlist"));
        detail_title.add_css_class("title-2");
        detail_title.set_halign(gtk::Align::Start);
        detail_box.append(&detail_title);

        let detail_subtitle = gtk::Label::new(Some(""));
        detail_subtitle.add_css_class("dim-label");
        detail_subtitle.set_halign(gtk::Align::Start);
        detail_box.append(&detail_subtitle);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_margin_top(12);
        actions.set_halign(gtk::Align::Start);

        let play_button = gtk::Button::with_label("Putar");
        play_button.add_css_class("suggested-action");
        play_button.update_property(&[gtk::accessible::Property::Label("Putar playlist")]);

        let shuffle_button = gtk::Button::with_label("Acak");
        shuffle_button.update_property(&[gtk::accessible::Property::Label("Acak playlist")]);

        actions.append(&play_button);
        actions.append(&shuffle_button);
        detail_box.append(&actions);

        let track_list = gtk::ListBox::new();
        track_list.add_css_class("playlist-tracks");
        track_list.set_selection_mode(gtk::SelectionMode::Multiple);

        let track_scroller = gtk::ScrolledWindow::builder()
            .child(&track_list)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .margin_top(12)
            .build();
        detail_box.append(&track_scroller);

        widget.append(&detail_box);

        let page = Self {
            content: widget.clone().upcast(),
            list,
            detail_title,
            detail_subtitle,
            play_button,
            shuffle_button,
            track_list,
            state,
        };

        page.connect_signals();
        page.connect_new_playlist(new_button);
        page.refresh();
        page
    }

    pub fn widget(&self) -> &gtk::Widget {
        &self.content
    }

    fn connect_signals(&self) {
        let state = Rc::clone(&self.state);
        self.play_button.connect_clicked(move |_| {
            let Some(playlist) = state.open_playlist() else {
                return;
            };
            let tracks = state.playlist_tracks(playlist.id);
            if !tracks.is_empty() {
                state.playback.play_tracks(&tracks, 0);
            }
        });

        let state = Rc::clone(&self.state);
        self.shuffle_button.connect_clicked(move |_| {
            let Some(playlist) = state.open_playlist() else {
                return;
            };
            let tracks = state.playlist_tracks(playlist.id);
            if !tracks.is_empty() {
                state.playback.set_shuffle(true);
                state.playback.play_tracks(&tracks, 0);
            }
        });
    }

    /// Muat ulang daftar playlist dan detailnya.
    pub fn refresh(&self) {
        let repo = PlaylistRepository::new(self.state.database.connection());
        let playlists = repo.list().unwrap_or_default();

        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }

        if playlists.is_empty() {
            let empty = gtk::Label::new(Some("Belum ada playlist"));
            empty.add_css_class("dim-label");
            empty.set_margin_top(12);
            self.list.append(&empty);
        }

        for playlist in &playlists {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);

            let box_widget = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            box_widget.set_margin_top(6);
            box_widget.set_margin_bottom(6);

            let name = gtk::Label::new(Some(&playlist.name));
            name.set_xalign(0.0);
            name.set_hexpand(true);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            box_widget.append(&name);

            let count = gtk::Label::new(Some(&format!("{}", playlist.track_count)));
            count.add_css_class("dim-label");
            count.add_css_class("caption");
            box_widget.append(&count);

            row.set_child(Some(&box_widget));

            let state = Rc::clone(&self.state);
            let playlist_id = playlist.id;
            row.connect_activate(move |_| {
                state.set_open_playlist(Some(playlist_id));
            });

            self.list.append(&row);
        }

        self.refresh_detail();
    }

    /// Muat ulang detail playlist yang sedang dibuka.
    fn refresh_detail(&self) {
        while let Some(child) = self.track_list.first_child() {
            self.track_list.remove(&child);
        }

        let Some(playlist) = self.state.open_playlist() else {
            self.detail_title.set_text("Pilih sebuah playlist");
            self.detail_subtitle.set_text("");
            self.play_button.set_sensitive(false);
            self.shuffle_button.set_sensitive(false);
            return;
        };

        let tracks = self.state.playlist_tracks(playlist.id);

        self.detail_title.set_text(&playlist.name);
        self.detail_subtitle.set_text(&format!(
            "{} lagu · {}",
            tracks.len(),
            format_duration(tracks.iter().map(|t| t.duration_ms).sum())
        ));
        self.play_button.set_sensitive(!tracks.is_empty());
        self.shuffle_button.set_sensitive(!tracks.is_empty());

        if tracks.is_empty() {
            let empty = gtk::Label::new(Some("Playlist ini masih kosong"));
            empty.add_css_class("dim-label");
            empty.set_margin_top(24);
            self.track_list.append(&empty);
            return;
        }

        for (index, track) in tracks.iter().enumerate() {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);

            let box_widget = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            box_widget.set_margin_top(4);
            box_widget.set_margin_bottom(4);

            let number = gtk::Label::new(Some(&(index + 1).to_string()));
            number.add_css_class("dim-label");
            number.set_size_request(28, -1);
            box_widget.append(&number);

            let cover = artwork::artwork_for_track(track, 36);
            box_widget.append(&cover);

            let labels = gtk::Box::new(gtk::Orientation::Vertical, 0);
            labels.set_hexpand(true);

            let title = gtk::Label::new(Some(track.display_title()));
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            labels.append(&title);

            let artist = gtk::Label::new(Some(track.display_artist()));
            artist.set_xalign(0.0);
            artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
            artist.add_css_class("dim-label");
            artist.add_css_class("caption");
            labels.append(&artist);

            box_widget.append(&labels);

            let duration = gtk::Label::new(Some(&format_duration(track.duration_ms)));
            duration.add_css_class("dim-label");
            duration.add_css_class("caption");
            box_widget.append(&duration);

            row.set_child(Some(&box_widget));

            let state = Rc::clone(&self.state);
            let track_clone = track.clone();
            row.connect_activate(move |_| {
                state.playback.play_single(&track_clone);
                state.mark_played(track_clone.id);
            });

            self.track_list.append(&row);
        }
    }

    /// Hubungkan tombol buat playlist dengan dialog nama.
    fn connect_new_playlist(&self, button: gtk::Button) {
        let state = Rc::clone(&self.state);
        button.connect_clicked(move |_| {
            let state = Rc::clone(&state);
            crate::ui::playlist_dialog::ask_playlist_name(
                &gtk::Window::builder().build(),
                "Buat Playlist",
                "Nama playlist:",
                move |name| {
                    let repo = PlaylistRepository::new(state.database.connection());
                    match repo.create(&name) {
                        Ok(_) => {
                            state.refresh_library();
                            state.set_status_message(format!("Playlist \"{name}\" dibuat."));
                        }
                        Err(err) => {
                            log::error!("Gagal membuat playlist: {err}");
                            state.set_status_message("Gagal membuat playlist.");
                        }
                    }
                },
            );
        });
    }

    /// Mainkan playlist yang sedang dibuka.
    pub fn play_current_playlist(&self, shuffle: bool) {
        let Some(playlist) = self.state.open_playlist() else {
            return;
        };
        let tracks: Vec<Track> = self.state.playlist_tracks(playlist.id);
        if tracks.is_empty() {
            return;
        }

        if shuffle {
            self.state.playback.set_shuffle(true);
        }
        self.state.playback.play_tracks(&tracks, 0);
    }
}
