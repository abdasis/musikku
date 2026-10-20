# MusikKu

Pemutar musik lokal untuk Linux desktop, dibangun dengan Rust, GTK4, dan libadwaita.

MusikKu berfokus pada koleksi musik lokal: memindai folder musik, membaca
metadata, memutar audio, mengelola playlist, dan menyediakan kontrol playback
yang konsisten dengan desktop Linux.

## Fitur

- **Library** — pemindaian folder musik rekursif, pembacaan metadata (ID3,
  Vorbis Comment, dan format terkait), cover album, pencarian, dan pengurutan.
- **Playback** — play/pause, previous/next, seek, volume, bisu, shuffle, dan
  repeat (off / all / one).
- **Queue** — antrean putar yang bertahan saat berpindah halaman, dapat
  ditambah, dihapus, dan diurutkan ulang.
- **Playlist** — buat, ganti nama, hapus, tambah/hapus lagu, putar, dan acak.
- **Integrasi Linux** — MPRIS untuk media control desktop, desktop entry, dan
  ikon aplikasi.
- **Tampilan** — dark mode bawaan, layout responsif, pintasan keyboard, dan
  dukungan aksesibilitas GTK.

## Kebutuhan Sistem

Paket berikut diperlukan untuk membangun (nama paket mengikuti Debian/Ubuntu):

```
build-essential
libgtk-4-dev
libadwaita-1-dev
libgstreamer1.0-dev
libgstreamer-plugins-base1.0-dev
libsqlite3-dev
pkg-config
```

GStreamer juga membutuhkan plugin decoder yang sesuai (mp3, flac, vorbis,
opus). Pada Debian/Ubuntu: `gstreamer1.0-plugins-good`,
`gstreamer1.0-plugins-bad`, `gstreamer1.0-libav`.

## Menjalankan

```bash
cargo run
```

Aplikasi menyimpan database dan cover album di
`$XDG_DATA_HOME/musikku/` (bawaan: `~/.local/share/musikku/`).

## Membangun

```bash
cargo build --release
```

Binari hasil berada di `target/release/musikku`.

## Pengujian

```bash
cargo test
```

Unit test mencakup format durasi, logika antrean, dan parsing path.
Integration test mencakup repository database: upsert, pengurutan, pencarian,
favorit, playlist, dan persistensi state playback.

## Struktur Proyek

```
src/
  core/        Model domain dan utilitas format
  database/    Koneksi SQLite, migrasi, dan repository
  library/     Pemindaian folder dan pembacaan metadata
  playback/    Mesin GStreamer, antrean, dan pengendali
  playlist/    (akan diisi) repository playlist
  settings/    Preferensi pengguna
  mpris/       Integrasi media control desktop
  ui/          Widget, halaman, dan jendela utama
  app/         State bersama dan siklus hidup aplikasi
```

Pemisahan ini menjaga agar lapisan I/O berat — pemindaian dan pembacaan
metadata — tidak pernah berjalan di thread utama GTK.

## Arsitektur

- **Thread utama** hanya menangani UI dan playback.
- **Thread pemindai** membaca berkas audio dan mengirim hasilnya lewat
  channel; database ditulis dari thread utama.
- **Thread async** menjalankan server MPRIS; perintahnya diteruskan kembali
  ke thread utama melalui `MainContext`.

## Pintasan Keyboard

| Pintasan | Aksi |
|---|---|
| Space | Play / Pause |
| Ctrl + F | Fokus pencarian |
| Ctrl + O | Buka file musik |
| Ctrl + Shift + N | Buat playlist |
| Ctrl + , | Pengaturan |
| Ctrl + Q | Keluar |
| Alt + Left / Right | Navigasi halaman |
| Ctrl + Right / Left | Lagu berikutnya / sebelumnya |

## Pemasangan

### Desktop entry

```bash
install -Dm644 data/io.github.musikku.MusikKu.desktop \
  ~/.local/share/applications/io.github.musikku.MusikKu.desktop
install -Dm644 resources/icons/io.github.musikku.MusikKu.svg \
  ~/.local/share/icons/hicolor/scalable/apps/io.github.musikku.MusikKu.svg
install -Dm644 data/io.github.musikku.MusikKu.metainfo.xml \
  ~/.local/share/metainfo/io.github.musikku.MusikKu.metainfo.xml
```

### Flatpak

Lihat `flatpak/io.github.musikku.MusikKu.yaml` untuk manifest distro-agnostic.

## Lisensi

GPL-3.0-or-later
