//! Widget cover album yang dipakai di beberapa halaman.

use std::path::Path;

use gtk::gdk;
use gtk::prelude::*;
use gtk::{Image, Picture};

use crate::core::model::Track;

/// Placeholder yang konsisten saat cover tidak tersedia.
const PLACEHOLDER_ICON: &str = "audio-x-generic-symbolic";

/// Buat widget cover album untuk sebuah track.
///
/// Cover dimuat dari disk lalu di-cache oleh GdkPixbuf.
pub fn artwork_for_track(track: &Track, size: i32) -> gtk::Widget {
    match track.artwork_path.as_ref() {
        Some(path) => artwork_for_path(path, size),
        None => placeholder(size),
    }
}

/// Buat widget cover album dari sebuah path berkas gambar.
///
/// Gambar dipotong menjadi persegi dengan ukuran tetap supaya tata letak
/// halaman tetap rapi.
pub fn artwork_for_path(path: &Path, size: i32) -> gtk::Widget {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    container.set_size_request(size, size);
    container.set_halign(gtk::Align::Center);
    container.set_valign(gtk::Align::Center);
    container.add_css_class("artwork-frame");

    match load_pixbuf(path, size) {
        Some(pixbuf) => {
            let texture = gdk::Texture::for_pixbuf(&pixbuf);
            let picture = Picture::for_paintable(&texture);
            picture.set_content_fit(gtk::ContentFit::Cover);
            picture.set_can_shrink(true);
            picture.set_halign(gtk::Align::Fill);
            picture.set_valign(gtk::Align::Fill);
            // Widget tidak boleh melebar mengikuti ukuran gambar asli.
            picture.set_size_request(size, size);
            picture.set_vexpand(false);
            picture.set_hexpand(false);
            container.append(&picture);
        }
        None => {
            // Berkas cover rusak: pakai placeholder, jangan gagal diam-diam.
            log::warn!("Cover gagal dimuat: {}", path.display());
            container.append(&placeholder_icon(size / 2));
        }
    }

    container.upcast()
}

/// Placeholder standar untuk cover yang tidak tersedia.
pub fn placeholder(size: i32) -> gtk::Widget {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    container.set_size_request(size, size);
    container.set_halign(gtk::Align::Center);
    container.set_valign(gtk::Align::Center);
    container.add_css_class("artwork-frame");
    container.append(&placeholder_icon(size / 2));

    container.upcast()
}

/// Ikon placeholder untuk cover yang tidak tersedia.
fn placeholder_icon(pixel_size: i32) -> gtk::Widget {
    let image = Image::from_icon_name(PLACEHOLDER_ICON);
    image.set_pixel_size(pixel_size.max(16));
    image.set_halign(gtk::Align::Center);
    image.set_valign(gtk::Align::Center);
    image.add_css_class("artwork-placeholder");
    image.upcast()
}

/// Muat gambar dari disk sebagai pixbuf dengan ukuran maksimal tertentu.
///
/// Gambar diperkecil sebelum ditampilkan supaya widget tidak ikut
/// membesar mengikuti resolusi asli.
fn load_pixbuf(path: &Path, size: i32) -> Option<gdk_pixbuf::Pixbuf> {
    if !path.is_file() {
        return None;
    }
    gdk_pixbuf::Pixbuf::from_file_at_scale(path, size, size, true).ok()
}
