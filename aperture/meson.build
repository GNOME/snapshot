aperture_cargo_options = [ '--manifest-path', meson.project_source_root() / 'aperture' / 'Cargo.toml' ]
aperture_cargo_options += [ '--target-dir', meson.project_build_root() / 'aperture' / 'src' ]

test (
  'Cargo clippy - aperture',
  cargo,
  args: [
    'clippy',
    '--no-deps',
    aperture_cargo_options,
  ],
  env: [
    cargo_env,
  ],
  timeout: 400, # cargo might take a bit of time sometimes
)
