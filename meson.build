project(
  'snapshot',
  'rust',
  version: '50.beta',
  meson_version: '>= 1.7',
  license: 'GPL-3.0-or-later',
)

i18n = import('i18n')
gnome = import('gnome')

base_id = 'org.gnome.Snapshot'

dependency('glib-2.0', version: '>= 2.75')
dependency('gtk4', version: '>= 4.18')
dependency('libadwaita-1', version: '>= 1.8.alpha')
dependency('glycin-2', version: '>= 2')
dependency('glycin-gtk4-2', version: '>= 2')
dependency('gstreamer-1.0', version: '>= 1.20')
dependency('gstreamer-video-1.0', version: '>= 1.20')
# Needed for camerabin
dependency('gstreamer-plugins-bad-1.0', version: '>= 1.20')
# Required by glycin crate
dependency('lcms2', version: '>=2.12.0')
dependency('libseccomp', version: '>= 2.5.0')

# Optional dependencies
gtk4_x11 = dependency('gtk4-x11', required: get_option('x11'))

glib_compile_resources = find_program('glib-compile-resources', required: true)
glib_compile_schemas = find_program('glib-compile-schemas', required: true)
desktop_file_validate = find_program('desktop-file-validate', required: false)
appstreamcli = find_program('appstreamcli', required: false)
cargo = find_program('cargo', required: true)

version = meson.project_version()

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
localedir = prefix / get_option('localedir')

datadir = prefix / get_option('datadir')
pkgdatadir = datadir / meson.project_name()
iconsdir = datadir / 'icons'
podir = meson.project_source_root() / 'po'
gettext_package = meson.project_name()

is_devel = get_option('profile') == 'development'

if is_devel
  message('Building in debug mode')

  profile = 'Devel'
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD', check: false).stdout().strip()
  if vcs_tag == ''
    version_suffix = '-devel'
  else
    version_suffix = '-@0@'.format(vcs_tag)
  endif
  application_id = '@0@.@1@'.format(base_id, profile)

  rust_target = 'debug'
  cargo_release_options = []
  clippy_deny_warnings = [
    '--',
    '--deny',
    'warnings',
  ]
else
  message('Building in release mode')

  profile = ''
  version_suffix = ''
  application_id = base_id

  rust_target = 'release'
  cargo_release_options = [ '--release' ]
  clippy_deny_warnings = []
endif

cargo_env = [
  'CARGO_HOME=' + meson.project_build_root() / 'cargo-home',
  'CARGO_TARGET_DIR=' + meson.project_build_root() / 'cargo-target',
]
meson.add_devenv(cargo_env)

meson.add_dist_script(
  'build-aux/dist-vendor.sh',
  meson.project_build_root() / 'meson-dist' / meson.project_name() + '-' + version,
  meson.project_source_root()
)

if is_devel
  # Setup pre-commit hook for ensuring coding style is always consistent
  message('Setting up git pre-commit hook..')
  run_command('cp', '-f', 'hooks/pre-commit.hook', '.git/hooks/pre-commit', check: false)
endif

clippy_options = []
clippy_options += cargo_release_options
clippy_options += [
  '--all-targets',
  '--all-features',
  '--workspace',
]
clippy_options += clippy_deny_warnings

subdir('data')
subdir('po')
subdir('src')
subdir('aperture')

test (
  'cargo-clippy',
  cargo,
  args: [
    'clippy',
    clippy_options,
  ],
  env: [
    cargo_env,
  ],
  timeout: 400, # cargo might take a bit of time sometimes
)

gnome.post_install(
  gtk_update_icon_cache: true,
  glib_compile_schemas: true,
  update_desktop_database: true,
)
