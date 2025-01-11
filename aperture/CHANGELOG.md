# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Changed
- Aperture now uses the rqrr crate instead of zbar
- Not all QR codes are valid UTF-8, therefore the `code-detected` signal now
  presents the contents as GBytes instead of String, it also does not have a
  data type parameter anymore

### Removed
- CodeType is not used anymore and was removed

## 0.8.0 - 2024-10-18

### Added
- A new changelog
- viewfinder: Add a `disable_audio_recording` property (!294)

### Changed
- Aperture now dynamically links with `gst-plugin-gtk4` from the host. Apps
  should require it as a dependency (!309)
- Only get devices from the `pipewiredeviceprovider` (!316)
- Optimize best mode selection for 16:9 (!317)
- Use GraphicsOffload to draw black background. This requires GTK 4.16 (!324)

### Fixed
- Correct `From<&str>` for `CodeType::Code128`

## 0.7.0 - 2024-07-17

### Added
- Run `cargo clippy` on CI
- Use `gtk::GraphicsOffload` for viewfinder. This requires GTK 4.14.

### Fixed
- Do not crash when linking pads

### Changed
- Improve caps selection

## 0.6.3 - 2024-05-09
## 0.6.2 - 2024-05-08
## 0.6.1 - 2024-03-15
## 0.6.0 - 2024-02-18
## 0.5.0 - 2024-02-11
## 0.4.1 - 2023-12-10
## 0.4.0 - 2023-12-02
## 0.3.2 - 2023-09-24
## 0.3.1 - 2023-08-11
## 0.3.0 - 2023-08-11
## 0.2.0 - 2023-06-04
## 0.1.0 - 2023-04-17
