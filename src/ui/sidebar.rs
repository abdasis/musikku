//! Sidebar navigasi utama.

use std::rc::Rc;

use adw::prelude::*;

use crate::app::state::{AppState, Page};

/// Item navigasi di sidebar.
struct NavItem {
    page: Page,
    row: adw::ActionRow,
}

/// Sidebar berisi navigasi utama dan aksi tambah folder musik.
pub struct Sidebar {
    widget: adw::NavigationPage,
    /// Baris yang sedang aktif, dipakai untuk menyorot item terpilih.
    rows: Vec<NavItem>,
    group: adw::PreferencesGroup,
}

impl Sidebar {
    pub fn new(state: Rc<AppState>) -> Self {
        let group = adw::PreferencesGroup::new();
        group.set_title("Navigasi");

        let mut rows = Vec::new();
        for page in [
            Page::Home,
            Page::Library,
            Page::Playlists,
            Page::Favorites,
            Page::RecentlyPlayed,
        ] {
            let row = adw::ActionRow::new();
            row.set_title(page.title());
            row.add_prefix(&gtk::Image::from_icon_name(page.icon_name()));
            row.set_activatable(true);

            let page_for_click = page;
            let state_for_click = Rc::clone(&state);
            row.connect_activated(move |_| {
                state_for_click.navigate_to(page_for_click);
            });

            group.add(&row);
            rows.push(NavItem { page, row });
        }

        // Aksi tambah folder musik, mudah ditemukan di bawah navigasi.
        let add_folder = adw::ActionRow::new();
        add_folder.set_title("Tambah Folder Musik");
        add_folder.set_subtitle("Pindai koleksi lokal kamu");
        add_folder.add_prefix(&gtk::Image::from_icon_name("folder-new-symbolic"));
        add_folder.set_activatable(true);
        group.add(&add_folder);

        let state_for_folder = Rc::clone(&state);
        add_folder.connect_activated(move |_| {
            state_for_folder.navigate_to(Page::Library);
            state_for_folder.set_status_message("Pilih folder musik untuk dipindai.");
        });

        let preferences = adw::PreferencesGroup::new();
        preferences.set_title("Aplikasi");

        let settings_row = adw::ActionRow::new();
        settings_row.set_title(Page::Settings.title());
        settings_row.add_prefix(&gtk::Image::from_icon_name(Page::Settings.icon_name()));
        settings_row.set_activatable(true);

        let state_for_settings = Rc::clone(&state);
        settings_row.connect_activated(move |_| {
            state_for_settings.navigate_to(Page::Settings);
        });

        preferences.add(&settings_row);
        rows.push(NavItem {
            page: Page::Settings,
            row: settings_row,
        });

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.append(&group);
        content.append(&preferences);

        let scroller = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();

        let widget = adw::NavigationPage::new(&scroller, "Menu");
        widget.set_tag(Some("sidebar"));

        Self {
            widget,
            rows,
            group,
        }
    }

    pub fn widget(&self) -> &adw::NavigationPage {
        &self.widget
    }

    /// Sorot item navigasi yang sesuai dengan halaman aktif.
    pub fn set_active_page(&self, page: Page) {
        for item in &self.rows {
            if item.page == page {
                item.row.add_css_class("selected");
            } else {
                item.row.remove_css_class("selected");
            }
        }
    }

    /// Jumlah folder musik yang terdaftar, ditampilkan di bagian bawah.
    pub fn set_folder_count(&self, count: usize) {
        self.group.set_description(Some(&format!(
            "{} folder musik terdaftar",
            count
        )));
    }
}
