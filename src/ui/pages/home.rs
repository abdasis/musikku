//! Halaman Home: ringkasan koleksi, lagu terakhir diputar, dan album favorit.

use std::rc::Rc;

use adw::prelude::*;

use crate::app::state::AppState;
use crate::core::model::Track;
use crate::core::time::format_relative;

use super::super::artwork;

/// Halaman landing yang merangkum koleksi pengguna.
pub struct HomePage {
    /// Isi utama halaman, ditampilkan saat library tidak kosong.
    content: gtk::Box,
    /// Versi widget yang sudah di-upcast, untuk disimpan ke container.
    view: gtk::Widget,
    /// Area kosong yang tampil ketika library belum berisi apa pun.
    empty_page: adw::StatusPage,
    /// Grid album dan playlist.
    album_grid: gtk::FlowBox,
    recently_box: gtk::FlowBox,
    state: Rc<AppState>,
}

impl HomePage {
    pub fn new(state: Rc<AppState>) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.add_css_class("home-page");

        // ---- Empty state ----
        let empty_page = adw::StatusPage::new();
        empty_page.set_title("Belum ada musik di library");
        empty_page.set_description(Some(
            "Tambahkan folder musik untuk mulai memutar koleksi lokal kamu.",
        ));
        empty_page.set_icon_name(Some("audio-x-generic-symbolic"));

        let add_folder = gtk::Button::with_label("Tambah Folder Musik");
        add_folder.add_css_class("suggested-action");
        add_folder.add_css_class("pill");
        empty_page.set_child(Some(&add_folder));

        widget.append(&empty_page);

        // ---- Konten utama ----
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_margin_top(18);
        content.set_margin_start(18);
        content.set_margin_end(18);
        content.set_margin_bottom(18);
        content.set_spacing(18);

        let welcome = gtk::Label::new(Some("Musik untuk suasana hati kamu"));
        welcome.add_css_class("title-1");
        welcome.set_halign(gtk::Align::Start);
        content.append(&welcome);

        let subtitle = gtk::Label::new(Some("Putar, kelola, dan nikmati koleksi musik lokal."));
        subtitle.add_css_class("dim-label");
        subtitle.set_halign(gtk::Align::Start);
        content.append(&subtitle);

        // Recently played.
        let recently_title = gtk::Label::new(Some("Terakhir Diputar"));
        recently_title.add_css_class("heading");
        recently_title.set_halign(gtk::Align::Start);
        content.append(&recently_title);

        // FlowBox membungkus kartu ke baris berikutnya saat ruang sempit.
        let recently_box = gtk::FlowBox::new();
        recently_box.set_selection_mode(gtk::SelectionMode::None);
        recently_box.set_homogeneous(true);
        recently_box.set_column_spacing(12);
        recently_box.set_row_spacing(12);
        recently_box.set_min_children_per_line(1);
        content.append(&recently_box);

        // Album favorit.
        let albums_title = gtk::Label::new(Some("Album Favorit"));
        albums_title.add_css_class("heading");
        albums_title.set_halign(gtk::Align::Start);
        content.append(&albums_title);

        let album_grid = gtk::FlowBox::new();
        album_grid.set_selection_mode(gtk::SelectionMode::None);
        album_grid.set_homogeneous(true);
        album_grid.set_column_spacing(12);
        album_grid.set_row_spacing(12);
        album_grid.set_min_children_per_line(1);
        content.append(&album_grid);

        widget.append(&content);

        let page = Self {
            view: widget.clone().upcast(),
            empty_page,
            content,
            album_grid,
            recently_box,
            state,
        };

        page.connect_signals();
        page
    }

    pub fn widget(&self) -> &gtk::Widget {
        &self.view
    }

    fn connect_signals(&self) {
        let state = Rc::clone(&self.state);

        if let Some(button) = self
            .empty_page
            .child()
            .and_then(|c| c.downcast::<gtk::Button>().ok())
        {
            button.connect_clicked(move |_| {
                state.set_status_message("Pilih folder musik untuk dipindai.");
                state.navigate_to(crate::app::state::Page::Library);
            });
        }
    }

    /// Muat ulang isi halaman.
    pub fn refresh(&self) {
        let snapshot = self.state.snapshot();
        let is_empty = snapshot.tracks.is_empty();

        self.empty_page.set_visible(is_empty);
        self.content.set_visible(!is_empty);

        if is_empty {
            return;
        }

        self.refresh_recently(&snapshot.tracks);
        self.refresh_albums(&snapshot.tracks);
    }

    /// Tampilkan album yang baru diputar, diurutkan dari yang terbaru.
    fn refresh_recently(&self, tracks: &[Track]) {
        clear_children(&self.recently_box);

        let mut recent: Vec<&Track> = tracks.iter().collect();
        recent.sort_by(|a, b| {
            b.last_played_at
                .cmp(&a.last_played_at)
                .then_with(|| a.display_title().cmp(b.display_title()))
        });

        let recent: Vec<&Track> = recent.into_iter().take(8).collect();
        if recent.is_empty() {
            let label = gtk::Label::new(Some("Belum ada lagu yang diputar"));
            label.add_css_class("dim-label");
            self.recently_box.append(&label);
            return;
        }

        for track in recent {
            let card = self.build_card(track);
            self.recently_box.append(&card);
        }
    }

    /// Tampilkan album favorit beserta jumlah lagunya.
    fn refresh_albums(&self, tracks: &[Track]) {
        clear_children(&self.album_grid);

        // Kelompokkan track berdasarkan album.
        let mut albums: Vec<(String, String, usize)> = Vec::new();
        for track in tracks {
            let key = format!("{}\u{1}{}", track.display_album(), track.display_artist());
            match albums.iter_mut().find(|(album, _artist, _)| *album == key) {
                Some(entry) => entry.2 += 1,
                None => albums.push((key, track.display_album().to_string(), 1)),
            }
        }

        albums.sort_by_key(|a| a.1.to_lowercase());
        albums.truncate(8);

        if albums.is_empty() {
            let label = gtk::Label::new(Some("Belum ada album"));
            label.add_css_class("dim-label");
            self.album_grid.append(&label);
            return;
        }

        for (key, album_name, count) in albums {
            let (album, artist) = key.split_once('\u{1}').unwrap_or((&album_name, ""));
            let cover = tracks
                .iter()
                .find(|t| t.display_album() == album)
                .and_then(|t| t.artwork_path.as_ref())
                .cloned();

            let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
            card.set_size_request(150, -1);

            let cover_widget = match cover {
                Some(path) => artwork::artwork_for_path(&path, 150),
                None => artwork::placeholder(150),
            };
            card.append(&cover_widget);

            let title = gtk::Label::new(Some(album_name.as_str()));
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.set_max_width_chars(16);
            card.append(&title);

            let subtitle = gtk::Label::new(Some(&format!("{artist} · {count} lagu")));
            subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
            subtitle.set_max_width_chars(18);
            subtitle.add_css_class("dim-label");
            subtitle.add_css_class("caption");
            card.append(&subtitle);

            self.album_grid.append(&card);
        }
    }

    /// Buat kartu satu lagu untuk bagian terakhir diputar.
    fn build_card(&self, track: &Track) -> gtk::Widget {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.set_size_request(150, -1);

        let cover = artwork::artwork_for_track(track, 150);
        card.append(&cover);

        let title = gtk::Label::new(Some(track.display_title()));
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(16);
        card.append(&title);

        let subtitle = gtk::Label::new(Some(track.display_artist()));
        subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
        subtitle.set_max_width_chars(18);
        subtitle.add_css_class("dim-label");
        subtitle.add_css_class("caption");
        card.append(&subtitle);

        let state = Rc::clone(&self.state);
        let track_for_click = track.clone();
        let button = gtk::Button::from_icon_name("media-playback-start-symbolic");
        button.set_tooltip_text(Some("Putar"));
        button.add_css_class("flat");
        button.update_property(&[gtk::accessible::Property::Label("Putar")]);
        button.connect_clicked(move |_| {
            state.playback.play_single(&track_for_click);
            state.mark_played(track_for_click.id);
        });
        card.append(&button);

        // Informasikan kapan lagu terakhir diputar.
        if let Some(played_at) = track.last_played_at {
            let time_label = gtk::Label::new(Some(&format_relative(played_at)));
            time_label.add_css_class("dim-label");
            time_label.add_css_class("caption");
            card.append(&time_label);
        }

        card.upcast()
    }
}

/// Hapus semua anak dari sebuah container.
fn clear_children(container: &gtk::FlowBox) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
