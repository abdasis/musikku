//! Dialog untuk membuat atau mengganti nama playlist.

use adw::prelude::*;

/// Tampilkan dialog yang meminta nama playlist.
///
/// Callback `on_confirm` dijalankan dengan nama yang sudah dibersihkan
/// dari spasi di awal dan akhir. Bila dibatalkan, callback tidak dipanggil.
pub fn ask_playlist_name(
    parent: Option<&adw::ApplicationWindow>,
    title: &str,
    label: &str,
    on_confirm: impl Fn(String) + 'static,
) {
    ask_playlist_name_with(parent, title, label, "", on_confirm)
}

/// Tampilkan dialog nama playlist dengan nilai awal.
pub fn ask_playlist_name_with(
    parent: Option<&adw::ApplicationWindow>,
    title: &str,
    label: &str,
    initial: &str,
    on_confirm: impl Fn(String) + 'static,
) {
    // AlertDialog bersifat modal secara bawaan.
    let dialog = adw::AlertDialog::new(Some(title), None);

    let entry = adw::EntryRow::new();
    entry.set_title(label);
    entry.set_text(initial);

    let group = adw::PreferencesGroup::new();
    group.add(&entry);
    dialog.set_extra_child(Some(&group));

    dialog.add_response("cancel", "Batal");
    dialog.add_response("create", "Buat");
    dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("create"));

    let entry_for_confirm = entry.clone();
    dialog.connect_response(None, move |dialog, response| {
        if response == "create" {
            let name = entry_for_confirm.text().trim().to_string();
            if !name.is_empty() {
                on_confirm(name);
            }
        }
        dialog.close();
    });

    dialog.present(parent);
}
