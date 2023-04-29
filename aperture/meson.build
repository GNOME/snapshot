cargo_options = [ '--manifest-path', meson.project_source_root() / 'aperture' / 'Cargo.toml' ]
cargo_options += [ '--target-dir', meson.project_build_root() / 'aperture' ]

doc_options = [
  '--',
  '-Z', 'unstable-options',
  '--extern-html-root-url=gtk4=https://gtk-rs.org/gtk4-rs/stable/latest/docs/',
  '--extern-html-root-url=gio=https://gtk-rs.org/gtk-rs-core/stable/latest/docs/',
  '--extern-html-root-url=glib=https://gtk-rs.org/gtk-rs-core/stable/latest/docs/',
  '--extern-html-root-url=gio=https://gtk-rs.org/gtk-rs-core/stable/latest/docs/',
  '--extern-html-root-url=gstreamer=https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/',
  '--default-theme=light'
]

cargo_doc = custom_target(
  'cargo-doc',
  build_by_default: false,
  build_always_stale: true,
  output: 'doc',
  console: true,
  command: [
    cargo, 'rustdoc',
    cargo_options + doc_options,
  ],
)