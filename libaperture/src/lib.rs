use std::os::raw::c_char;

use gtk::glib;

mod camera;
mod device_provider;
mod viewfinder;

pub use camera::ApertureCamera;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aperture_init(app_id: *const c_char) {
    let app_id = unsafe { glib::GStr::from_ptr(app_id) };
    aperture::init(app_id.as_str());
}
