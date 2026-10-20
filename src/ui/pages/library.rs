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

        // ---- Baris aksi multi-selection ----
        // Muncul hanya ketika ada lagu yang dipilih.
        let selection_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        selection_bar.set_margin_start(16);
        selection_bar.set_margin_end(16);
        selection_bar.set_margin_bottom(8);
        selection_bar.set_visible(false);
        selection_bar.add_css_class("selection-bar");

        let selection_label = gtk::Label::new(Some("0 lagu dipilih"));
        selection_label.add_css_class("heading");
        selection_label.set_hexpand(true);
        selection_bar.append(&selection_label);

        let enqueue_button = gtk::Button::with_label("Tambah ke Antrean");
        enqueue_button.add_css_class("flat");
        enqueue_button.update_property(&[gtk::accessible::Property::Label("Tambah ke Antrean")]);
        selection_bar.append(&enqueue_button);

        let favorite_button = gtk::Button::with_label("Favorit");
        favorite_button.add_css_class("flat");
        favorite_button.update_property(&[gtk::accessible::Property::Label("Tandai favorit")]);
        selection_bar.append(&favorite_button);

        let remove_button = gtk::Button::with_label("Hapus dari Library");
        remove_button.add_css_class("flat");
        remove_button.add_css_class("destructive-action");
        remove_button.update_property(&[gtk::accessible::Property::Label("Hapus dari library")]);
        selection_bar.append(&remove_button);

        widget.append(&selection_bar);

        // ---- Tabel lagu ----
        let model = TrackListModel::new();

        // Aksi tabel: putar, tambah ke antrean, playlist, dan hapus.
        let state_for_play = Rc::clone(&state);
        let state_for_menu = Rc::clone(&state);
        let model_for_menu = model.clone();
        let table = build_track_table(
            &model,
            super::super::track_list::TrackTableActions {
                on_play: Box::new(move |track| {
                    state_for_play.playback.play_single(track);
                    state_for_play.mark_played(track.id);
                }),
                on_selection_changed: Box::new({
                    let bar = selection_bar.clone();
                    let label = selection_label.clone();
                    move |selected| {
                        let count = selected.len();
                        bar.set_visible(count > 0);
                        label.set_text(&if count == 1 {
                            "1 lagu dipilih".to_string()
                        } else {
                            format!("{count} lagu dipilih")
                        });
                    }
                }),
                on_context_menu: Box::new(move |track, popover| {
                    let state = Rc::clone(&state_for_menu);

                    // Bila ada beberapa lagu terpilih, aksi berlaku untuk
                    // semuanya; selain itu hanya untuk baris yang diklik.
                    let tracks = model_for_menu.selected_tracks();
                    let tracks = if tracks.len() > 1 {
                        tracks
                    } else {
                        vec![track.clone()]
                    };

                    super::super::context_menu::show_track_menu(&state, &tracks, popover);
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
        page.connect_bulk_actions(enqueue_button, favorite_button, remove_button);
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

    /// Hubungkan aksi massal pada baris multi-selection.
    fn connect_bulk_actions(
        &self,
        enqueue_button: gtk::Button,
        favorite_button: gtk::Button,
        remove_button: gtk::Button,
    ) {
        {
            let state = Rc::clone(&self.state);
            let model = self.model.clone();
            enqueue_button.connect_clicked(move |_| {
                let tracks = model.selected_tracks();
                if tracks.is_empty() {
                    return;
                }
                state.playback.enqueue_many(&tracks);
                state.set_status_message(format!("{} lagu ditambahkan ke antrean.", tracks.len()));
            });
        }

        {
            let state = Rc::clone(&self.state);
            let model = self.model.clone();
            favorite_button.connect_clicked(move |_| {
                let tracks = model.selected_tracks();
                if tracks.is_empty() {
                    return;
                }
                for track in &tracks {
                    state.set_favorite(track.id, true);
                }
                state.refresh_library();
                state
                    .set_status_message(format!("{} lagu ditandai sebagai favorit.", tracks.len()));
            });
        }

        {
            let state = Rc::clone(&self.state);
            let model = self.model.clone();
            remove_button.connect_clicked(move |_| {
                let tracks = model.selected_tracks();
                if tracks.is_empty() {
                    return;
                }

                // Menghapus track hanya menghapus baris database; berkas
                // audio di disk tetap ada.
                let repo = crate::database::TrackRepository::new(state.database.connection());
                let mut removed = 0;
                for track in &tracks {
                    if repo.remove(track.id).is_ok() {
                        state.playback.remove_from_queue(track.id);
                        removed += 1;
                    }
                }

                state.refresh_library();
                state.set_status_message(format!(
                    "{removed} lagu dihapus dari library. Berkas tetap tersimpan."
                ));
            });
        }
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
