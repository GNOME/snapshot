global_conf = configuration_data()
global_conf.set_quoted('APP_ID', application_id)
global_conf.set_quoted('PKGDATADIR', pkgdatadir)
global_conf.set_quoted('PROFILE', profile)
if is_devel
  global_conf.set('IS_DEVEL', 'true')
else
  global_conf.set('IS_DEVEL', 'false')
endif
global_conf.set_quoted('VERSION', version + version_suffix)
global_conf.set_quoted('GETTEXT_PACKAGE', gettext_package)
global_conf.set_quoted('LOCALEDIR', localedir)

config = configure_file(
  input: 'config.rs.in',
  output: 'config.rs',
  configuration: global_conf
)
# Copy the config.rs output to the source directory.
run_command(
  'cp',
  meson.project_build_root() / 'src' / 'config.rs',
  meson.project_source_root() / 'src' / 'config.rs',
  check: true
)

cargo_build_options = [ '--manifest-path', meson.project_source_root() / 'Cargo.toml' ]

if gtk4_x11.found()
  cargo_build_options += [ '--features', 'x11' ]
endif

cargo_build_options += cargo_release_options

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  depends: resources,
  env: cargo_env,
  command: [
    cargo, 
    'build',
    cargo_build_options,
    '&&',
    'cp', 'cargo-target' / rust_target / meson.project_name(), '@OUTPUT@',
  ]
)
