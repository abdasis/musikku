//! Halaman Favorit dan Recently Played, keduanya memakai tabel lagu yang sama.

use std::rc::Rc;

use gtk::prelude::*;

use crate::app::state::AppState;
use crate::core::model::Track;
use crate::database::{SortDirection, TrackFilter, TrackRepository, TrackSort};

use super::super::track_list::{build_track_table, TrackListModel};

/// Halaman yang menampilkan kumpulan lagu memakai tabel standar.
pub struct CollectionPage {
    /// Versi widget yang sudah di-upcast.
    content: gtk::Widget,
    model: TrackListModel,
    /// Filter yang dipakai saat memuat data.
    filter: TrackFilter,
    /// Judul halaman.
    title: String,
    state: Rc<AppState>,
}

impl CollectionPage {
    /// Buat halaman untuk track yang ditandai favorit.
    pub fn new_favorites(state: Rc<AppState>) -> Self {
        Self::new(
            state,
            TrackFilter {
                favorites_only: true,
                ..Default::default()
            },
            "Favorit",
        )
    }

    /// Buat halaman untuk lagu yang terakhir diputar.
    pub fn new_recently_played(state: Rc<AppState>) -> Self {
        Self::new(state, TrackFilter::default(), "Terakhir Diputar")
    }

    fn new(state: Rc<AppState>, filter: TrackFilter, title: &str) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.add_css_class("collection-page");

        let model = TrackListModel::new();
        let state_for_actions = Rc::clone(&state);
        let table = build_track_table(
            &model,
            super::super::track_list::TrackTableActions {
                on_play: Box::new(move |track| {
                    state_for_actions.playback.play_single(track);
                    state_for_actions.mark_played(track.id);
                }),
                on_selection_changed: Box::new(|_| {}),
                on_context_menu: Box::new(|_track, _popover| {}),
            },
        );
        widget.append(&table);

        let page = Self {
            content: widget.clone().upcast(),
            model,
            filter,
            title: title.to_string(),
            state,
        };

        page.refresh();
        page
    }

    pub fn widget(&self) -> &gtk::Widget {
        &self.content
    }

    /// Judul halaman.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Muat ulang isi tabel.
    pub fn refresh(&self) {
        let repo = TrackRepository::new(self.state.database.connection());
        let mut tracks = repo
            .query(
                &self.filter,
                TrackSort::Title,
                SortDirection::Ascending,
            )
            .unwrap_or_default();

        if self.filter.favorites_only {
            tracks.retain(|t| t.is_favorite);
        } else {
            // Halaman "terakhir diputar" hanya menampilkan lagu yang pernah diputar.
            tracks.retain(|t| t.last_played_at.is_some());
            tracks.sort_by_key(|a| std::cmp::Reverse(a.last_played_at));
        }

        self.model.set_tracks(tracks);
    }

    /// Track yang sedang dipilih.
    pub fn selected_tracks(&self) -> Vec<Track> {
        self.model.selected_tracks()
    }
}
