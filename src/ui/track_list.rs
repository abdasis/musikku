//! Model daftar lagu untuk halaman Library, Favorit, dan Recently Played.
//!
//! Memakai `GtkListStore` + `GtkColumnView` supaya koleksi besar tetap
//! lancar: GTK hanya membuat widget untuk baris yang terlihat.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk::gdk;
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
    /// Baris yang dipilih, dikelola sendiri karena GtkMultiSelection
    /// hanya menyeleksi satu baris pada satu waktu.
    selected: Rc<RefCell<Vec<u32>>>,
    /// Listener yang diberi tahu saat seleksi berubah.
    listeners: Rc<RefCell<Vec<SelectionListener>>>,
}

/// Listener yang diberi tahu saat seleksi berubah.
type SelectionListener = Box<dyn Fn(Vec<Track>)>;

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
            selected: Rc::new(RefCell::new(Vec::new())),
            listeners: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Daftarkan listener yang diberi tahu saat seleksi berubah.
    pub fn add_selection_listener(&self, listener: impl Fn(Vec<Track>) + 'static) {
        self.listeners.borrow_mut().push(Box::new(listener));
    }

    /// Beri tahu listener bahwa seleksi berubah.
    ///
    /// Notifikasi ditunda ke idle supaya tidak berjalan di dalam signal
    /// GTK yang sedang memegang borrow model seleksi.
    fn notify_selection_changed(&self) {
        let model = self.clone();
        glib::idle_add_local_once(move || {
            let tracks = model.selected_tracks();
            let listeners = model.listeners.borrow();
            for listener in listeners.iter() {
                listener(tracks.clone());
            }
        });
    }

    /// Seleksi yang berlaku, dibaca dari daftar milik model.
    fn current_selection(&self) -> Vec<u32> {
        self.selected.borrow().clone()
    }

    /// Tambahkan atau hapus baris dari seleksi.
    ///
    /// Mengembalikan `true` bila baris sekarang terpilih.
    pub fn toggle_selected(&self, position: u32) -> bool {
        let mut selected = self.selected.borrow().clone();
        let is_selected = match selected.iter().position(|p| *p == position) {
            Some(index) => {
                selected.remove(index);
                false
            }
            None => {
                selected.push(position);
                true
            }
        };

        if let Ok(mut current) = self.selected.try_borrow_mut() {
            *current = selected.clone();
        }

        // GtkMultiSelection menambah baris lewat select_range; dengan
        // begitu baris sebelumnya tidak dibatalkan.
        if is_selected {
            // unselect_rest = false: baris sebelumnya tetap terpilih.
            self.selection.select_range(position, 1, false);
        } else {
            self.selection.unselect_item(position);
        }

        self.notify_selection_changed();
        is_selected
    }

    /// Tambahkan baris ke seleksi tanpa membatalkan baris lain.
    ///
    /// Dipakai untuk klik dengan modifier Ctrl, yang selalu menambah
    /// baris baru meski baris itu sebelumnya sudah terpilih.
    pub fn add_selected(&self, position: u32) {
        let mut selected = self.selected.borrow().clone();
        if !selected.contains(&position) {
            selected.push(position);
        }

        if let Ok(mut current) = self.selected.try_borrow_mut() {
            *current = selected.clone();
        }

        // Seleksi kumulatif ditulis ke model GTK supaya baris yang
        // disorot di tabel sama dengan yang tercatat di model.
        let bitset = gtk::Bitset::new_empty();
        for row in selected.iter() {
            bitset.add(*row);
        }
        let mask = gtk::Bitset::new_range(0, self.store.n_items());
        self.selection.set_selection(&bitset, &mask);

        self.notify_selection_changed();
    }

    /// Pilih satu baris saja dan ganti seleksi sebelumnya.
    pub fn select_only(&self, position: u32) {
        if let Ok(mut selected) = self.selected.try_borrow_mut() {
            *selected = vec![position];
        }

        // Seleksi ditulis ke model GTK supaya sorotannya sesuai.
        let bitset = gtk::Bitset::new_range(position, 1);
        let mask = gtk::Bitset::new_range(0, self.store.n_items());
        self.selection.set_selection(&bitset, &mask);

        self.notify_selection_changed();
    }

    /// Kosongkan seleksi.
    pub fn clear_selection(&self) {
        if let Ok(mut selected) = self.selected.try_borrow_mut() {
            selected.clear();
        }
        self.selection.unselect_all();
    }

    /// Baris yang sedang dipilih.
    pub fn selected_positions(&self) -> Vec<u32> {
        self.selected.borrow().clone()
    }

    /// Ganti daftar baris terpilih dan beri tahu listener.
    ///
    /// Dipakai saat seleksi GTK berubah, supaya model dan tampilan
    /// selalu sama. Penulisan memakai `try_borrow_mut` karena signal GTK
    /// dapat masuk kembali saat seleksi model sedang ditulis.
    pub fn set_selected_positions(&self, positions: Vec<u32>) {
        self.apply_selection(positions, true);
    }

    /// Terapkan seleksi baru dari GTK ke daftar model.
    ///
    /// GtkMultiSelection hanya menyimpan baris terakhir, jadi baris baru
    /// digabung dengan daftar yang ada. Seleksi lama dibuang ketika
    /// pengguna mengklik tanpa modifier, yang ditandai dengan
    /// `exclusive`.
    pub fn apply_selection(&self, positions: Vec<u32>, exclusive: bool) {
        if let Ok(mut selected) = self.selected.try_borrow_mut() {
            if exclusive {
                selected.clear();
            }
            for position in positions {
                if !selected.contains(&position) {
                    selected.push(position);
                }
            }
        } else {
            return;
        }
        self.notify_selection_changed();
    }

    /// Gabungkan baris terpilih dari GTK dengan yang sudah ada.
    ///
    /// GtkMultiSelection hanya melaporkan baris terakhir yang diklik dan
    /// mengganti seleksi sebelumnya, jadi seleksi kumulatif disimpan
    /// sendiri di model.
    pub fn merge_selected_positions(&self, positions: Vec<u32>) {
        if let Ok(mut selected) = self.selected.try_borrow_mut() {
            for position in positions {
                if !selected.contains(&position) {
                    selected.push(position);
                }
            }
        } else {
            return;
        }
        self.notify_selection_changed();
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
    ///
    /// Daftar dibaca dari seleksi milik model sendiri karena
    /// GtkMultiSelection hanya menyimpan satu baris terpilih.
    pub fn selected_tracks(&self) -> Vec<Track> {
        let mut selected = Vec::new();
        for position in self.current_selection() {
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

/// Cari indeks baris yang berada di bawah titik tertentu.
///
/// Titik koordinat relatif terhadap widget yang menerimanya. Widget yang
/// ter-pick bisa berada di dalam sel, jadi rantai induknya diperiksa
/// sampai ketemu sel yang tercatat.
fn row_under_pointer(
    widget: &gtk::Widget,
    cell_positions: &RefCell<HashMap<gtk::Widget, Rc<std::cell::Cell<u32>>>>,
    x: f64,
    y: f64,
) -> Option<u32> {
    let picked = widget.pick(x, y, gtk::PickFlags::DEFAULT)?;

    let mut current = Some(picked);
    while let Some(widget) = current {
        if let Some(position) = cell_positions.borrow().get(&widget) {
            return Some(position.get());
        }
        current = widget.parent();
    }

    None
}

/// Bangun widget tabel lagu lengkap dengan kolom dan factory.
pub fn build_track_table(model: &TrackListModel, actions: TrackTableActions) -> gtk::Widget {
    let column_view = gtk::ColumnView::new(Some(model.selection().clone()));
    column_view.set_show_row_separators(true);
    column_view.set_show_column_separators(false);

    // Model dipakai bersama oleh seluruh kolom dan gesture.
    let shared_model = Rc::new(model.clone());

    // Aksi menu konteks dipakai seluruh kolom.
    let on_context_menu = Rc::new(actions.on_context_menu);

    // Gesture per sel, dipakai untuk melepas gesture lama saat sel
    // dipakai ulang untuk baris lain.
    let cell_gestures: Rc<RefCell<HashMap<gtk::Widget, gtk::GestureClick>>> =
        Rc::new(RefCell::new(HashMap::new()));

    // Posisi baris per sel, diperbarui setiap kali baris diikat.
    let cell_positions: Rc<RefCell<HashMap<gtk::Widget, Rc<std::cell::Cell<u32>>>>> =
        Rc::new(RefCell::new(HashMap::new()));

    for column in TrackColumn::ALL {
        let factory = gtk::SignalListItemFactory::new();
        let property = column.property_name();
        let is_artwork = column == TrackColumn::Artwork;

        factory.connect_setup(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };

            // Isi sel dibungkus kotak yang mengisi sel, sehingga klik
            // di area kosong pada baris tetap mengenai sel.
            let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            container.set_hexpand(true);
            container.set_vexpand(true);

            if is_artwork {
                container.append(&super::artwork::placeholder(36));
            } else {
                let label = gtk::Label::new(None);
                label.set_xalign(0.0);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.set_hexpand(true);
                container.append(&label);
            }

            item.set_child(Some(&container));
        });

        let property_for_bind = property.to_string();
        factory.connect_bind({
            let model = Rc::clone(&shared_model);
            let on_context_menu = Rc::clone(&on_context_menu);
            let column_view_for_gesture = column_view.clone();
            let cell_gestures = Rc::clone(&cell_gestures);
            let cell_positions = Rc::clone(&cell_positions);

            move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                    return;
                };
                let Some(cell) = item.child() else {
                    return;
                };

                let position = item.position();
                {}

                // Sel menyimpan posisi barisnya sendiri. Posisi ini
                // selalu diperbarui saat baris diikat, sehingga nilainya
                // selalu sesuai baris yang sedang ditampilkan.
                {
                    let mut map = cell_positions.borrow_mut();
                    let entry = map
                        .entry(cell.clone())
                        .or_insert_with(|| Rc::new(std::cell::Cell::new(0)))
                        .clone();
                    entry.set(position);

                    // Anak sel ikut dicatat karena `pick` mengembalikan
                    // widget paling dalam, mis. label.
                    let mut stack = vec![cell.clone()];
                    while let Some(widget) = stack.pop() {
                        let mut current = widget.first_child();
                        while let Some(descendant) = current {
                            map.insert(descendant.clone(), entry.clone());
                            stack.push(descendant.clone());
                            current = descendant.next_sibling();
                        }
                    }
                }

                // Isi sel diperbarui tanpa mengganti kotaknya.
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

                    if let Some(container) = cell.downcast_ref::<gtk::Box>() {
                        while let Some(child) = container.first_child() {
                            container.remove(&child);
                        }
                        container.append(&artwork);
                    }
                } else if let Some(label) = cell
                    .first_child()
                    .and_then(|c| c.downcast::<gtk::Label>().ok())
                {
                    let value = item
                        .item()
                        .map(|obj| obj.property::<String>(&property_for_bind))
                        .unwrap_or_default();
                    label.set_text(&value);
                }

                // Gesture dibuat ulang setiap baris diikat supaya posisi
                // yang dibawa selalu sesuai. Gesture lama dilepas lebih
                // dulu supaya tidak menumpuk.
                if let Some(previous) = cell_gestures.borrow_mut().remove(&cell) {
                    cell.remove_controller(&previous);
                }

                // Klik kanan menampilkan menu konteks untuk baris ini.
                let right = gtk::GestureClick::new();
                right.set_button(gdk::BUTTON_SECONDARY);
                let model_for_right = Rc::clone(&model);
                let on_context_menu = Rc::clone(&on_context_menu);
                let column_view_for_right = column_view_for_gesture.clone();
                let cell_for_right = cell.clone();
                let cell_positions = Rc::clone(&cell_positions);
                right.connect_pressed(move |gesture, n_press, x, y| {
                    if n_press != 1 {
                        return;
                    }

                    // Posisi baris diambil dari ListItem sel, karena
                    // widget sel dipakai ulang antar baris.
                    let row = cell_positions
                        .borrow()
                        .get(&cell_for_right)
                        .map(|position| position.get())
                        .unwrap_or(position);

                    let Some(track) = model_for_right.track_at(row) else {
                        return;
                    };

                    // Koordinat gesture relatif terhadap sel; popover
                    // diposisikan relatif terhadap tabel.
                    let origin = cell_for_right
                        .compute_point(&column_view_for_right, &graphene::Point::new(0.0, 0.0))
                        .unwrap_or_else(|| graphene::Point::new(0.0, 0.0));

                    let popover = gtk::Popover::new();
                    popover.set_parent(&column_view_for_right);
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
                cell.add_controller(right);
            }
        });

        let view_column = gtk::ColumnViewColumn::new(Some(column.title()), Some(factory));
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

    // Klik pada tabel: baris biasa mengganti seleksi, klik dengan
    // modifier Ctrl menambah baris ke seleksi.
    {
        let model = Rc::clone(&shared_model);
        let cell_positions = Rc::clone(&cell_positions);

        let column_view_for_pick = column_view.clone();

        // Event legacy dipakai karena koordinat gesture tidak merujuk
        // posisi kursor, sedangkan event asli memilikinya.
        let controller = gtk::EventControllerLegacy::new();
        controller.connect_event(move |_controller, event| {
            let Some(button_event) = event.downcast_ref::<gdk::ButtonEvent>() else {
                return glib::Propagation::Proceed;
            };
            if button_event.button() != gdk::BUTTON_PRIMARY {
                return glib::Propagation::Proceed;
            }

            let Some((table_x, table_y)) = button_event.position() else {
                return glib::Propagation::Proceed;
            };

            // Baris ditentukan dari sel yang ter-pick.
            let column_view: &gtk::Widget = column_view_for_pick.as_ref();
            let Some(row) = row_under_pointer(column_view, &cell_positions, table_x, table_y)
            else {
                return glib::Propagation::Proceed;
            };

            let modifiers = button_event.modifier_state();
            let additive = modifiers.contains(gdk::ModifierType::CONTROL_MASK)
                || modifiers.contains(gdk::ModifierType::SHIFT_MASK);

            if additive {
                model.add_selected(row);
            } else {
                model.select_only(row);
            }

            glib::Propagation::Stop
        });
        column_view.add_controller(controller);
    }

    // Klik ganda memutar lagu pada baris tersebut.
    {
        let model = Rc::clone(&shared_model);
        let on_play = actions.on_play;
        column_view.connect_activate(move |_view, position| {
            if let Some(track) = model.track_at(position) {
                on_play(&track);
            }
        });
    }

    // Perubahan seleksi dipakai untuk multi-selection. Seleksi GTK
    // disalin ke model agar listener melihat keadaan yang sama.
    {
        let model = Rc::clone(&shared_model);
        let on_selection_changed = actions.on_selection_changed;
        model.add_selection_listener(move |selected| {
            on_selection_changed(selected);
        });

        // Sinyal GTK hanya bounced-back dari tulisan model sendiri,
        // jadi perubahan dari GTK tidak menimpa daftar model.
        // Seleksi GTK disalin ke model agar listener dan operasi
        // massal melihat baris yang sama dengan yang disorot di tabel.
        let selection_model = model.selection().clone();
        let previous = Rc::new(std::cell::Cell::new(Vec::<u32>::new()));
        selection_model.connect_selection_changed(move |selection, _, _| {
            let bitset = selection.selection();
            let mut positions = Vec::new();
            if let Some((mut iter, _)) = gtk::BitsetIter::init_first(&bitset) {
                for position in iter.by_ref() {
                    positions.push(position);
                }
            }

            // Sinyal GTK hanya dipakai untuk menyorot baris; daftar
            // kumulatif dikelola model.
            let _ = previous.replace(positions.clone());
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
