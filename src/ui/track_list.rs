//! Model daftar lagu untuk halaman Library, Favorit, dan Recently Played.
//!
//! Memakai `GtkListStore` + `GtkColumnView` supaya koleksi besar tetap
//! lancar: GTK hanya membuat widget untuk baris yang terlihat.

use std::rc::Rc;

use gtk::gdk;
use gtk::graphene;
use gtk::gio;
use gtk::prelude::*;

use crate::core::model::Track;

use super::track_row::TrackRow;

/// Kolom model daftar lagu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackColumn {
    Artwork,
    Title,
    Artist,
    Album,
    Duration,
}

impl TrackColumn {
    /// Urutan kolom pada tabel.
    pub const ALL: [TrackColumn; 5] = [
        TrackColumn::Artwork,
        TrackColumn::Title,
        TrackColumn::Artist,
        TrackColumn::Album,
        TrackColumn::Duration,
    ];

    /// Nama properti pada baris lagu.
    fn property_name(self) -> &'static str {
        match self {
            TrackColumn::Artwork => "artwork-path",
            TrackColumn::Title => "title",
            TrackColumn::Artist => "artist",
            TrackColumn::Album => "album",
            TrackColumn::Duration => "duration",
        }
    }

    /// Judul kolom tabel.
    pub fn title(self) -> &'static str {
        match self {
            TrackColumn::Artwork => "",
            TrackColumn::Title => "Judul",
            TrackColumn::Artist => "Artis",
            TrackColumn::Album => "Album",
            TrackColumn::Duration => "Durasi",
        }
    }
}

/// Sumber data daftar lagu.
#[derive(Clone)]
pub struct TrackListModel {
    store: Rc<gio::ListStore>,
    selection: Rc<gtk::MultiSelection>,
}

impl TrackListModel {
    /// Buat model kosong.
    pub fn new() -> Self {
        let store = gio::ListStore::builder()
            .item_type(TrackRow::static_type())
            .build();

        let selection = gtk::MultiSelection::new(Some(store.clone()));

        Self {
            store: Rc::new(store),
            selection: Rc::new(selection),
        }
    }

    /// Ganti seluruh isi daftar.
    pub fn set_tracks(&self, tracks: Vec<Track>) {
        self.store.remove_all();

        for track in tracks {
            self.store.append(&TrackRow::from_track(&track));
        }
    }

    /// Track pada posisi tertentu.
    pub fn track_at(&self, position: u32) -> Option<Track> {
        self.store
            .item(position)
            .and_then(|item| item.downcast::<TrackRow>().ok())
            .and_then(|row| row.track())
    }

    /// Track yang sedang dipilih.
    pub fn selected_tracks(&self) -> Vec<Track> {
        let selection = self.selection.selection();
        let Some((iter, _)) = gtk::BitsetIter::init_first(&selection) else {
            return Vec::new();
        };

        let mut selected = Vec::new();
        for position in iter {
            if let Some(track) = self.track_at(position) {
                selected.push(track);
            }
        }
        selected
    }

    /// Jumlah track yang dipilih.
    pub fn selected_count(&self) -> usize {
        self.selected_tracks().len()
    }

    /// Model selection yang dipakai ColumnView.
    pub fn selection(&self) -> &gtk::MultiSelection {
        &self.selection
    }
}

impl Default for TrackListModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Callback yang dipanggil saat menu konteks diminta.
pub type ContextMenuHandler = Box<dyn Fn(&Track, &gtk::Popover)>;

/// Aksi yang dapat dipicu dari interaksi pada tabel lagu.
pub struct TrackTableActions {
    /// Dipanggil saat satu baris diklik ganda: putar lagu tersebut.
    pub on_play: Box<dyn Fn(&Track)>,
    /// Dipanggil saat baris dipilih: untuk multi-selection.
    pub on_selection_changed: Box<dyn Fn(Vec<Track>)>,
    /// Dipanggil saat menu konteks diminta pada baris tertentu.
    pub on_context_menu: ContextMenuHandler,
}

/// Bangun widget tabel lagu lengkap dengan kolom dan factory.
pub fn build_track_table(model: &TrackListModel, actions: TrackTableActions) -> gtk::Widget {
    let column_view = gtk::ColumnView::new(Some(model.selection().clone()));

    // Aksi dibagikan ke seluruh kolom factory.
    let on_context_menu = Rc::new(actions.on_context_menu);
    column_view.set_show_row_separators(true);
    column_view.set_show_column_separators(false);

    for column in TrackColumn::ALL {
        let factory = gtk::SignalListItemFactory::new();
        let property = column.property_name();
        let is_artwork = column == TrackColumn::Artwork;

        factory.connect_setup(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            if is_artwork {
                item.set_child(Some(&super::artwork::placeholder(36)));
            } else {
                let label = gtk::Label::new(None);
                label.set_xalign(0.0);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                item.set_child(Some(&label));
            }
        });

        let property_for_bind = property.to_string();
        factory.connect_bind({
            let model = model.clone();
            let on_context_menu = Rc::clone(&on_context_menu);
            let column_view_for_gesture = column_view.clone();

            move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                    return;
                };

                if is_artwork {
                    let path = item
                        .item()
                        .map(|obj| obj.property::<String>(&property_for_bind))
                        .unwrap_or_default();

                    let artwork = if path.is_empty() {
                        super::artwork::placeholder(36)
                    } else {
                        super::artwork::artwork_for_path(std::path::Path::new(&path), 36)
                    };
                    item.set_child(Some(&artwork));
                } else {
                    let Some(label) = item.child().and_then(|c| c.downcast::<gtk::Label>().ok())
                    else {
                        return;
                    };

                    let value = item
                        .item()
                        .map(|obj| obj.property::<String>(&property_for_bind))
                        .unwrap_or_default();
                    label.set_text(&value);
                }

                // Klik kanan pada sel menampilkan menu konteks untuk baris
                // tersebut. Gesture dipasang pada widget sel karena itu
                // yang dikenai klik.
                let Some(cell) = item.child() else {
                    return;
                };

                let position = item.position();
                let cell_for_origin = cell.clone();
                let gesture = gtk::GestureClick::new();
                gesture.set_button(gdk::BUTTON_SECONDARY);

                let model = model.clone();
                let on_context_menu = Rc::clone(&on_context_menu);
                let column_view_for_gesture = column_view_for_gesture.clone();

                gesture.connect_pressed(move |gesture, n_press, x, y| {
                    if n_press != 1 {
                        return;
                    }

                    let Some(track) = model.track_at(position) else {
                        return;
                    };

                    // Koordinat gesture relatif terhadap sel; popover
                    // diposisikan relatif terhadap tabel.
                    let origin = cell_for_origin
                        .compute_point(
                            &column_view_for_gesture,
                            &graphene::Point::new(0.0, 0.0),
                        )
                        .unwrap_or_else(|| graphene::Point::new(0.0, 0.0));

                    let popover = gtk::Popover::new();
                    popover.set_parent(&column_view_for_gesture);
                    popover.set_has_arrow(false);
                    popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(
                        (origin.x() + x as f32) as i32,
                        (origin.y() + y as f32) as i32,
                        1,
                        1,
                    )));
                    on_context_menu(&track, &popover);
                    popover.popup();
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                });

                cell.add_controller(gesture);
            }
        });

        let view_column =
            gtk::ColumnViewColumn::new(Some(column.title()), Some(factory));
        view_column.set_resizable(!matches!(column, TrackColumn::Artwork));
        view_column.set_expand(matches!(column, TrackColumn::Title | TrackColumn::Artist));

        // Pengurutan memakai properti pada baris, bukan posisi list.
        let sorter = gtk::CustomSorter::new(move |a, b| {
            let a_value = a.property::<String>(property).to_lowercase();
            let b_value = b.property::<String>(property).to_lowercase();
            a_value.cmp(&b_value).into()
        });
        view_column.set_sorter(Some(&sorter));

        column_view.append_column(&view_column);
    }

    column_view.set_hexpand(true);
    column_view.set_vexpand(true);

    // Klik ganda memutar lagu pada baris tersebut.
    {
        let model = model.clone();
        let on_play = actions.on_play;
        column_view.connect_activate(move |_view, position| {
            if let Some(track) = model.track_at(position) {
                on_play(&track);
            }
        });
    }

    // Perubahan seleksi dipakai untuk multi-selection.
    {
        let model = model.clone();
        let on_selection_changed = actions.on_selection_changed;
        let selection_model = model.selection().clone();
        selection_model.connect_selection_changed(move |selection, _, _| {
            let bitset = selection.selection();
            let mut selected = Vec::new();
            if let Some((mut iter, _)) = gtk::BitsetIter::init_first(&bitset) {
                for position in iter.by_ref() {
                    if let Some(track) = model.track_at(position) {
                        selected.push(track);
                    }
                }
            }
            on_selection_changed(selected);
        });
    }

    let scroller = gtk::ScrolledWindow::builder()
        .child(&column_view)
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .build();
    scroller.add_css_class("library-scroller");

    scroller.upcast()
}
