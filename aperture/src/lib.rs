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

use std::sync::{Once, OnceLock};

use gst::prelude::*;

mod camera;
mod device_provider;
mod enums;
mod error;
mod pipeline_tee;
mod utils;
mod viewfinder;

pub use camera::Camera;
pub use device_provider::DeviceProvider;
pub use enums::{CameraLocation, CodeType, ViewfinderState};
pub use error::{CaptureError, PipewireError, ProviderError};
use once_cell::sync::Lazy;
pub(crate) use pipeline_tee::PipelineTee;
pub use viewfinder::Viewfinder;

pub(crate) static APP_ID: OnceLock<&'static str> = OnceLock::new();
pub(crate) const SUPPORTED_ENCODINGS: [&str; 2] = ["video/x-raw", "image/jpeg"];
/// The maximum framerate, in frames per second.
pub(crate) const MAXIMUM_RATE: i32 = 30;

/// Supported caps for the app, already frame capped.
pub(crate) static SUPPORTED_CAPS: Lazy<gst::Caps> = Lazy::new(|| {
    crate::SUPPORTED_ENCODINGS
        .iter()
        .map(|encoding| {
            gst_video::VideoCapsBuilder::for_encoding(*encoding)
                .framerate_range(
                    gst::Fraction::new(0, 1)..=gst::Fraction::new(crate::MAXIMUM_RATE, 1),
                )
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
pub fn init(app_id: &'static str) {
    IS_INIT.call_once(|| {
        APP_ID.set(app_id).unwrap();

        gtk::init().expect("Unable to start GTK");
        gst::init().expect("Failed to initalize gst");
        gstgtk4::plugin_register_static().expect("Failed to initalize gstgtk4");

        Viewfinder::static_type();
        DeviceProvider::static_type();
        Camera::static_type();

        CodeType::static_type();
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
        panic!("Aperture is not initialized! Please call `init()` before using the rest of the library to avoid errors and crashes.");
    }
}
