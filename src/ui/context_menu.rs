//! Menu konteks untuk track pada tabel library.

use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;

use crate::app::state::AppState;
use crate::core::model::Track;
use crate::database::PlaylistRepository;

/// Tampilkan menu konteks untuk sebuah track.
///
/// Menu dibangun dari tombol biasa agar perilakunya konsisten di semua
/// desktop dan mudah diakses keyboard.
pub fn show_track_menu(state: &Rc<AppState>, track: &Track, popover: &gtk::Popover) {
    let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu_box.set_margin_top(6);
    menu_box.set_margin_bottom(6);
    menu_box.set_margin_start(6);
    menu_box.set_margin_end(6);

    // ---- Putar dan antrean ----
    {
        let state = Rc::clone(state);
        let track = track.clone();
        let button = menu_button("Putar");
        let popover = popover.clone();
        button.connect_clicked(move |_| {
            state.playback.play_single(&track);
            state.mark_played(track.id);
            popover.popdown();
        });
        menu_box.append(&button);
    }

    {
        let state = Rc::clone(state);
        let track = track.clone();
        let button = menu_button("Tambahkan ke Antrean");
        let popover = popover.clone();
        button.connect_clicked(move |_| {
            state.playback.enqueue(&track);
            state.set_status_message(format!("{} ditambahkan ke antrean.", track.display_title()));
            popover.popdown();
        });
        menu_box.append(&button);
    }

    // ---- Tambahkan ke playlist ----
    let playlists: Vec<String> = PlaylistRepository::new(state.database.connection())
        .list()
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.name)
        .collect();

    if !playlists.is_empty() {
        menu_box.append(&separator());

        for playlist in &playlists {
            let state = Rc::clone(state);
            let track = track.clone();
            let name = playlist.clone();
            let button = menu_button(&format!("Tambahkan ke \"{name}\""));
            let popover = popover.clone();
            button.connect_clicked(move |_| {
                let repo = PlaylistRepository::new(state.database.connection());
                // Playlist dicari ulang berdasarkan nama karena id tidak
                // dibawa ke dalam closure.
                let Some(playlist) = repo
                    .list()
                    .unwrap_or_default()
                    .into_iter()
                    .find(|p| p.name == name)
                else {
                    return;
                };
                match repo.add_track(playlist.id, track.id) {
                    Ok(()) => {
                        state.refresh_library();
                        state.set_status_message(format!(
                            "{} ditambahkan ke playlist {}.",
                            track.display_title(),
                            name
                        ));
                    }
                    Err(err) => {
                        log::error!("Gagal menambah track ke playlist: {err}");
                        state.set_status_message("Gagal menambahkan lagu ke playlist.");
                    }
                }
                popover.popdown();
            });
            menu_box.append(&button);
        }
    }

    // ---- Aksi lainnya ----
    menu_box.append(&separator());

    {
        let state = Rc::clone(state);
        let track = track.clone();
        let label = if track.is_favorite {
            "Hapus dari Favorit"
        } else {
            "Tandai sebagai Favorit"
        };
        let button = menu_button(label);
        let popover = popover.clone();
        button.connect_clicked(move |_| {
            state.set_favorite(track.id, !track.is_favorite);
            state.refresh_library();
            state.set_status_message(if track.is_favorite {
                format!("{} dihapus dari favorit.", track.display_title())
            } else {
                format!("{} ditandai sebagai favorit.", track.display_title())
            });
            popover.popdown();
        });
        menu_box.append(&button);
    }

    {
        let track = track.clone();
        let button = menu_button("Tampilkan di Manajer Berkas");
        let popover = popover.clone();
        button.connect_clicked(move |_| {
            reveal_in_file_manager(&track);
            popover.popdown();
        });
        menu_box.append(&button);
    }

    {
        let state = Rc::clone(state);
        let track = track.clone();
        let button = menu_button("Hapus dari Library");
        let popover = popover.clone();
        button.connect_clicked(move |_| {
            // Menghapus track hanya menghapus baris database; berkas tetap ada.
            let repo = crate::database::TrackRepository::new(state.database.connection());
            match repo.remove(track.id) {
                Ok(()) => {
                    state.playback.remove_from_queue(track.id);
                    state.refresh_library();
                    state.set_status_message(format!(
                        "{} dihapus dari library. Berkas tetap tersimpan.",
                        track.display_title()
                    ));
                }
                Err(err) => {
                    log::error!("Gagal menghapus track: {err}");
                    state.set_status_message("Gagal menghapus lagu dari library.");
                }
            }
            popover.popdown();
        });
        menu_box.append(&button);
    }

    popover.set_child(Some(&menu_box));
}

/// Tombol menu dengan gaya rata kiri.
fn menu_button(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.set_halign(gtk::Align::Start);
    button.add_css_class("flat");
    button
}

/// Garis pemisah antar kelompok aksi.
fn separator() -> gtk::Separator {
    gtk::Separator::new(gtk::Orientation::Horizontal)
}

/// Buka folder induk track di manajer berkas.
///
/// Memakai `gio::AppInfo` agar mengikuti aplikasi bawaan sistem.
fn reveal_in_file_manager(track: &Track) {
    let Some(parent) = track.file_path.parent() else {
        return;
    };

    let file = gio::File::for_path(parent);
    let app = gio::AppInfo::default_for_type("inode/directory", false);

    match app {
        Some(app) => {
            let _ = app.launch(&[file], gio::AppLaunchContext::NONE);
        }
        None => {
            log::warn!("Tidak ada aplikasi untuk membuka folder musik");
        }
    }
}
