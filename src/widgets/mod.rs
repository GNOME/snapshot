// SPDX-License-Identifier: GPL-3.0-or-later
use gtk::prelude::*;

mod camera;
mod camera_row;
mod gallery;
mod gallery_button;
mod preferences_window;
mod shutter_button;
mod window;

pub use camera::Camera;
pub use camera_row::CameraRow;
pub use gallery::Gallery;
pub use gallery_button::GalleryButton;
pub use preferences_window::PreferencesWindow;
pub use shutter_button::ShutterButton;
pub use window::Window;

pub fn init() {
    Camera::static_type();
    Gallery::static_type();
    GalleryButton::static_type();
    ShutterButton::static_type();
}
