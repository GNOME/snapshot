// SPDX-License-Identifier: GPL-3.0-or-later

//! # libaperture
//!
//! GTK Widget for cameras using GStreamer and Pipewire
//!
//! See also
//!
//! - [Snapshot](https://gitlab.gnome.org/GNOME/snapshot)
//!
//! # Usage
//!
//! Aperture needs to initialized before use.
//! This can be done by calling [`fn@init`] on
//! [`startup`](fn@gtk::gio::prelude::ApplicationExt::connect_startup).

use std::sync::{LazyLock, Once, OnceLock};

use gst::prelude::*;

mod camera;
mod code_detector;
mod device_provider;
mod enums;
mod error;
mod pipeline_tee;
mod utils;
mod viewfinder;

pub use camera::Camera;
pub use device_provider::DeviceProvider;
pub use enums::{CameraLocation, VideoFormat, ViewfinderState};
pub use error::{CaptureError, PipewireError, ProviderError};
pub(crate) use pipeline_tee::PipelineTee;
pub use utils::{is_h264_encoding_supported, is_hardware_encoding_supported};
pub use viewfinder::Viewfinder;

pub(crate) static APP_ID: OnceLock<&'static str> = OnceLock::new();
pub(crate) const SUPPORTED_ENCODINGS: [&str; 2] = ["video/x-raw", "image/jpeg"];
/// The maximum framerate, in frames per second.
pub(crate) const MAXIMUM_RATE: i32 = 30;

/// Supported caps for the app, already frame capped.
pub(crate) static SUPPORTED_CAPS: LazyLock<gst::Caps> = LazyLock::new(|| {
    crate::SUPPORTED_ENCODINGS
        .iter()
        .map(|enc| {
            let framerate_range = gst::FractionRange::new(
                gst::Fraction::new(0, 1),
                gst::Fraction::new(crate::MAXIMUM_RATE, 1),
            );
            gst::Caps::builder(*enc)
                .field("framerate", framerate_range)
                .build()
        })
        .collect()
});

static IS_INIT: Once = Once::new();
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initializes the library
///
/// This function can be used instead of [`fn@gtk::init`] and [`fn@gst::init`]
/// as it initializes GTK and GStreamer implicitly.
///
/// This function must be called on the
/// [`startup`](fn@gtk::gio::prelude::ApplicationExt::connect_startup)
/// of the [`GApplication`][`gtk::gio::Application`]. This function is
/// idempotent.
///
/// # Panics
///
/// If it is not possible to initialize GTK, GST, and gst-rust-plugins.
pub fn init(app_id: &'static str) {
    IS_INIT.call_once(|| {
        APP_ID.set(app_id).unwrap();

        gtk::init().expect("Unable to start GTK");
        gst::init().expect("Failed to initialize gst");

        if let Err(err) = check_plugins() {
            log::warn!("{err:#}");
        }

        Viewfinder::static_type();
        DeviceProvider::static_type();
        Camera::static_type();

        ViewfinderState::static_type();
        CameraLocation::static_type();
    });
}

/// Gets the current version of Aperture
///
/// # Returns
///
/// The Aperture version
pub fn version() -> &'static str {
    VERSION
}

/// Use this function to check if Aperture has been initialized with
/// [`init()`][crate::init()].
///
/// # Panics
///
/// if Aperture is not initialized
pub(crate) fn ensure_init() {
    if !IS_INIT.is_completed() {
        panic!(
            "Aperture is not initialized! Please call `init()` before using the rest of the library to avoid errors and crashes."
        );
    }
}

// Check if all GStreamer plugins we require are available
fn check_plugins() -> Result<(), String> {
    let needed = ["camerabin", "gtk4", "pipewire", "videorate"];

    let registry = gst::Registry::get();

    let missing = needed
        .iter()
        .filter(|n| registry.find_plugin(n).is_none())
        .collect::<Vec<_>>();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Features might be missing due to missing gstreamer plugins: {missing:?}"
        ))
    }
}
