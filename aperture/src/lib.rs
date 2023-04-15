// SPDX-License-Identifier: GPL-3.0-or-later
use gst::prelude::StaticType;
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
pub use error::CaptureError;
pub use viewfinder::Viewfinder;

pub(crate) use pipeline_tee::PipelineTee;

pub(crate) static APP_ID: OnceCell<&'static str> = OnceCell::new();

static IS_INIT: Once = Once::new();

/// Initializes the library
///
/// Has to be called on the `startup` of the GApplication. This function is
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

pub(crate) fn ensure_init() {
    if !IS_INIT.is_completed() {
        panic!("Aperture is not initialized! Please call `init()` before using the rest of the library to avoid errors and crashes.");
    }
}
