use std::os::raw::c_char;

use glib::translate::Borrowed;
use gst::prelude::DeviceExt;
use gtk::glib;
use gtk::glib::translate::*;
use gtk::subclass::prelude::*;

use crate::Camera;

pub type ApertureCamera = <crate::camera::imp::Camera as ObjectSubclass>::Instance;

#[no_mangle]
pub unsafe extern "C" fn aperture_camera_get_display_name(
    self_ptr: *mut ApertureCamera,
) -> *const c_char {
    let obj: glib::translate::Borrowed<Camera> = from_glib_borrow(self_ptr);
    obj.display_name().as_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn aperture_camera_get_properties(
    self_ptr: *mut ApertureCamera,
) -> *mut gst::ffi::GstStructure {
    let obj: Borrowed<Camera> = from_glib_borrow(self_ptr);
    obj.device().properties().to_glib_full()
}
