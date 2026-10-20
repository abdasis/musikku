//! Menu konteks untuk track pada tabel library.

use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;

use crate::app::state::AppState;
use crate::core::model::Track;
use crate::database::PlaylistRepository;

/// Tampilkan menu konteks untuk sekumpulan track terpilih.
///
/// Bila lebih dari satu track dipilih, aksi berlaku untuk seluruh
///選 selected. Menu dibangun dari tombol biasa agar perilakunya konsisten
/// di semua desktop dan mudah diakses keyboard.
pub fn show_track_menu(state: &Rc<AppState>, tracks: &[Track], popover: &gtk::Popover) {
    let Some(first) = tracks.first() else {
        return;
    };
    let track = first.clone();
    let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu_box.set_margin_top(6);
    menu_box.set_margin_bottom(6);
    menu_box.set_margin_start(6);
    menu_box.set_margin_end(6);

    // ---- Putar dan antrean ----
    {
        let state = Rc::clone(state);
        let button = menu_button(if tracks.len() > 1 {
            "Putar Semua"
        } else {
            "Putar"
        });
        let popover = popover.clone();
        let tracks = tracks.to_vec();
        button.connect_clicked(move |_| {
            if tracks.len() == 1 {
                state.playback.play_single(&tracks[0]);
                state.mark_played(tracks[0].id);
            } else {
                state.playback.play_tracks(&tracks, 0);
            }
            popover.popdown();
        });
        menu_box.append(&button);
    }

    {
        let state = Rc::clone(state);
        let button = menu_button("Tambahkan ke Antrean");
        let popover = popover.clone();
        let tracks = tracks.to_vec();
        button.connect_clicked(move |_| {
            state.playback.enqueue_many(&tracks);
            state.set_status_message(format!("{} lagu ditambahkan ke antrean.", tracks.len()));
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
            let name = playlist.clone();
            let button = menu_button(&format!("Tambahkan ke \"{name}\""));
            let popover = popover.clone();
            let tracks = tracks.to_vec();
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

                let mut added = 0;
                for track in &tracks {
                    if repo.add_track(playlist.id, track.id).is_ok() {
                        added += 1;
                    }
                }
                state.refresh_library();
                state.set_status_message(format!("{added} lagu ditambahkan ke playlist {}.", name));
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
        let removing = track.is_favorite;
        let label = if removing {
            "Hapus dari Favorit"
        } else {
            "Tandai sebagai Favorit"
        };
        let button = menu_button(label);
        let popover = popover.clone();
        let tracks = tracks.to_vec();
        button.connect_clicked(move |_| {
            for track in &tracks {
                state.set_favorite(track.id, !removing);
            }
            state.refresh_library();
            state.set_status_message(if removing {
                format!("{} lagu dihapus dari favorit.", tracks.len())
            } else {
                format!("{} lagu ditandai sebagai favorit.", tracks.len())
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
        let button = menu_button("Hapus dari Library");
        let popover = popover.clone();
        let tracks = tracks.to_vec();
        button.connect_clicked(move |_| {
            // Menghapus track hanya menghapus baris database; berkas tetap ada.
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
