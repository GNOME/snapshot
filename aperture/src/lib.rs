// SPDX-License-Identifier: GPL-3.0-or-later

//! # libaperture
//!
//! GTK Widget for cameras using GStreamer and Pipewire
//!
//! See also
//!
//! - [Snapshot](https://gitlab.gnome.org/GNOME/Incubator/snapshot)
//!
//! # Usage
//!
//! Aperture needs to initialized before use.
//! This can be done by calling [`fn@init`] on [`startup`](fn@gtk::gio::prelude::ApplicationExt::connect_startup).

use gst::prelude::*;
use gtk::glib;
use once_cell::sync::OnceCell;
use std::sync::Once;

mod camera;
mod device_provider;
mod enums;
mod error;
mod pipeline_tee;
mod viewfinder;

pub use camera::Camera;
pub use device_provider::DeviceProvider;
pub use enums::{CameraLocation, CodeType, ViewfinderState};
pub use error::{CaptureError, PipewireError};
pub use viewfinder::Viewfinder;

pub(crate) use pipeline_tee::PipelineTee;

pub(crate) static APP_ID: OnceCell<&'static str> = OnceCell::new();

static IS_INIT: Once = Once::new();
static VERSION: &'static str = env!("CARGO_PKG_VERSION");

/// Initializes the library
///
/// This function can be used instead of [`fn@gtk::init`] and [`fn@gst::init`]
/// as it initializes GTK and GStreamer implicitly.
///
/// This function must be called on the [`startup`](fn@gtk::gio::prelude::ApplicationExt::connect_startup)
/// of the [`GApplication`][`gtk::gio::Application`]. This function is idempotent.
pub fn init(app_id: &'static str) {
    IS_INIT.call_once(|| {
        APP_ID.set(app_id).unwrap();

        gtk::init().expect("Unable to start GTK");
        gst::init().expect("Failed to initalize gst");
        gstgtk4::plugin_register_static().expect("Failed to initalize gstgtk4");

        if !gst::Registry::get().check_feature_version("pipewiresrc", 0, 3, 69) {
            let ctx = glib::MainContext::default();
            ctx.spawn_local(async move {
                if ashpd::is_sandboxed().await {
                    log::warn!("Pipewire version is too old, please run 'flatpak update'");
                } else {
                    log::warn!("Pipewire version is too old, please update to 0.3.69 or newer");
                }
            });
        }

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
    &*VERSION
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
