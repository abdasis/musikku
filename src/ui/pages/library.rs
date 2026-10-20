//! Halaman Library: tabel lagu dengan pencarian, pengurutan, dan menu konteks.

use std::rc::Rc;

use adw::prelude::*;

use crate::app::state::AppState;
use crate::core::model::Track;
use crate::database::{SortDirection, TrackFilter, TrackRepository, TrackSort};

use super::super::track_list::{build_track_table, TrackListModel};

/// Halaman library.
pub struct LibraryPage {
    /// Versi widget yang sudah di-upcast.
    content: gtk::Widget,
    model: TrackListModel,
    search_entry: gtk::SearchEntry,
    /// Kolom yang sedang disortir.
    sort_column: std::cell::Cell<TrackSort>,
    sort_descending: std::cell::Cell<bool>,
    state: Rc<AppState>,
    /// Panel status untuk pesan error dan hasil pencarian.
    status_label: gtk::Label,
}

impl LibraryPage {
    pub fn new(state: Rc<AppState>) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.add_css_class("library-page");

        // ---- Baris alat: pencarian dan jumlah lagu ----
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toolbar.set_margin_start(16);
        toolbar.set_margin_end(16);
        toolbar.set_margin_top(12);
        toolbar.set_margin_bottom(8);

        let search_entry = gtk::SearchEntry::new();
        search_entry.set_hexpand(true);
        search_entry.set_placeholder_text(Some("Cari lagu, artis, atau album"));
        search_entry.set_tooltip_text(Some("Cari lagu (Ctrl+F)"));
        search_entry.update_property(&[gtk::accessible::Property::Label("Cari lagu")]);
        toolbar.append(&search_entry);

        let status_label = gtk::Label::new(None);
        status_label.add_css_class("dim-label");
        status_label.add_css_class("caption");
        toolbar.append(&status_label);

        widget.append(&toolbar);

        // ---- Tabel lagu ----
        let model = TrackListModel::new();

        // Aksi tabel: putar, tambah ke antrean, playlist, dan hapus.
        let state_for_play = Rc::clone(&state);
        let state_for_menu = Rc::clone(&state);
        let table = build_track_table(
            &model,
            super::super::track_list::TrackTableActions {
                on_play: Box::new(move |track| {
                    state_for_play.playback.play_single(track);
                    state_for_play.mark_played(track.id);
                }),
                on_selection_changed: Box::new(|_| {}),
                on_context_menu: Box::new(move |track, popover| {
                    let state = Rc::clone(&state_for_menu);
                    let track = track.clone();
                    super::super::context_menu::show_track_menu(&state, &track, popover);
                }),
            },
        );
        widget.append(&table);

        let page = Self {
            content: widget.clone().upcast(),
            model,
            search_entry,
            sort_column: std::cell::Cell::new(TrackSort::Title),
            sort_descending: std::cell::Cell::new(false),
            state,
            status_label,
        };

        page.connect_signals();
        page.refresh();
        page
    }

    pub fn widget(&self) -> &gtk::Widget {
        &self.content
    }

    /// Fokuskan field pencarian.
    pub fn focus_search(&self) {
        self.search_entry.grab_focus();
    }

    fn connect_signals(&self) {
        let state = Rc::clone(&self.state);

        self.search_entry.connect_search_changed(move |entry| {
            let text = entry.text().to_string();
            state.set_status_message(if text.trim().is_empty() {
                String::new()
            } else {
                format!("Mencari \"{text}\"")
            });
        });

        // Muat ulang daftar setiap kali teks pencarian berubah.
        let state = Rc::clone(&self.state);
        self.search_entry.connect_search_changed(move |_| {
            state.refresh_library();
        });
    }

    /// Muat ulang isi tabel dari database memakai filter aktif.
    pub fn refresh(&self) {
        let search = self.search_entry.text().to_string();
        let filter = TrackFilter {
            search: if search.trim().is_empty() {
                None
            } else {
                Some(search.clone())
            },
            ..Default::default()
        };

        let repo = TrackRepository::new(self.state.database.connection());
        let tracks = repo
            .query(
                &filter,
                self.sort_column.get(),
                if self.sort_descending.get() {
                    SortDirection::Descending
                } else {
                    SortDirection::Ascending
                },
            )
            .unwrap_or_default();

        let count = tracks.len();
        self.model.set_tracks(tracks);

        if !search.trim().is_empty() {
            self.status_label
                .set_text(&format!("{count} lagu ditemukan"));
        } else {
            self.status_label.set_text(&format!("{count} lagu"));
        }
    }

    /// Ubah kolom pengurutan.
    pub fn set_sort(&mut self, column: TrackSort) {
        if self.sort_column.get() == column {
            let descending = !self.sort_descending.get();
            self.sort_descending.set(descending);
        } else {
            self.sort_column.set(column);
            self.sort_descending.set(false);
        }
        self.refresh();
    }

    /// Track yang sedang dipilih pada tabel.
    pub fn selected_tracks(&self) -> Vec<Track> {
        self.model.selected_tracks()
    }
}
