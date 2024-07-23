// SPDX-License-Identifier: GPL-3.0-or-later
use gtk::prelude::*;

mod camera;
mod camera_controls;
mod camera_row;
mod flash_bin;
mod gallery;
mod gallery_button;
mod gallery_item;
mod gallery_picture;
mod gallery_video;
mod guidelines_bin;
mod preferences_window;
mod qr_bottom_sheet;
mod qr_screen_bin;
mod shutter_button;
mod sliding_view;
mod video_player;
mod window;

pub use camera::Camera;
pub use camera_controls::CameraControls;
pub use camera_row::CameraRow;
pub use flash_bin::FlashBin;
pub use gallery::Gallery;
pub use gallery_button::GalleryButton;
pub use gallery_item::GalleryItem;
pub use gallery_picture::GalleryPicture;
pub use gallery_video::GalleryVideo;
pub use guidelines_bin::GuidelinesBin;
pub use preferences_window::PreferencesWindow;
pub use qr_bottom_sheet::QrBottomSheet;
pub use qr_screen_bin::QrScreenBin;
pub use shutter_button::ShutterButton;
pub use sliding_view::SlidingView;
pub use video_player::VideoPlayer;
pub use window::Window;

pub fn init() {
    Camera::static_type();
    Gallery::static_type();
    GalleryButton::static_type();
    GalleryPicture::static_type();
    ShutterButton::static_type();
    GalleryItem::static_type();
    FlashBin::static_type();
    SlidingView::static_type();
}
