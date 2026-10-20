//! Dialog pemilihan folder dan berkas musik memakai chooser native GTK.

use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;

use crate::app::state::AppState;

/// Buka dialog chooser native untuk memilih satu folder musik.
///
/// Callback `on_chosen` dijalankan di thread utama setelah folder dipilih.
pub fn choose_music_folder(
    parent: &impl IsA<gtk::Window>,
    state: Rc<AppState>,
    on_chosen: impl Fn(std::path::PathBuf) + 'static,
) {
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Pilih Folder Musik");
    dialog.set_modal(true);

    let state = Rc::clone(&state);
    dialog.select_folder(Some(parent), None::<&gio::Cancellable>, move |result| {
        let file = match result {
            Ok(file) => file,
            Err(err) => {
                // Pengguna menutup dialog tanpa memilih apa pun: bukan error.
                if !err.matches(gio::IOErrorEnum::Cancelled) {
                    log::warn!("Pemilihan folder gagal: {err}");
                }
                return;
            }
        };

        let Some(path) = file.path() else {
            state.set_status_message("Folder yang dipilih tidak dapat diakses.");
            return;
        };

        match state.library.add_folder(&path) {
            Ok(_) => {
                state.set_status_message(format!("Folder {} ditambahkan.", path.display()));
                on_chosen(path);
            }
            Err(err) => {
                log::error!("Gagal menambahkan folder: {err}");
                state.set_status_message("Gagal menambahkan folder musik.");
            }
        }
    });
}

/// Buka dialog chooser native untuk memilih beberapa berkas audio.
pub fn choose_audio_files(
    parent: &impl IsA<gtk::Window>,
    on_chosen: impl Fn(Vec<std::path::PathBuf>) + 'static,
) {
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Buka File Musik");
    dialog.set_modal(true);

    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Berkas audio"));
    for extension in crate::library::scanner::AUDIO_EXTENSIONS {
        filter.add_suffix(extension);
    }

    // FileDialog menerima daftar filter dalam bentuk ListModel.
    let filters = gio::ListStore::builder()
        .item_type(gtk::FileFilter::static_type())
        .build();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));

    dialog.open_multiple(Some(parent), None::<&gio::Cancellable>, move |result| {
        let files = match result {
            Ok(files) => files,
            Err(err) => {
                if !err.matches(gio::IOErrorEnum::Cancelled) {
                    log::warn!("Pemilihan berkas gagal: {err}");
                }
                return;
            }
        };

        let mut paths = Vec::new();
        for index in 0..files.n_items() {
            if let Some(file) = files.item(index) {
                if let Some(path) = file.downcast_ref::<gio::File>().and_then(|f| f.path()) {
                    paths.push(path);
                }
            }
        }

        if !paths.is_empty() {
            on_chosen(paths);
        }
    });
}
