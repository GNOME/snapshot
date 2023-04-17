cargo_options = [ '--manifest-path', meson.project_source_root() / 'aperture' / 'Cargo.toml' ]
cargo_options += [ '--target-dir', meson.project_build_root() / 'aperture' ]

cargo_doc = custom_target(
  'cargo-doc',
  build_by_default: false,
  build_always_stale: true,
  output: 'doc',
  console: true,
  command: [
    cargo, 'doc',
    cargo_options + ['--no-deps'],
  ],
)