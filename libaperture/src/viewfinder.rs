use std::os::raw::c_char;

use gtk::glib;
use gtk::glib::translate::*;
use gtk::subclass::prelude::*;

use aperture::Viewfinder;

pub type ApertureViewfinder = <aperture::ViewfinderInner as ObjectSubclass>::Instance;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aperture_viewfinder_new() -> *mut ApertureViewfinder {
    Viewfinder::new().to_glib_full()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aperture_viewfinder_take_picture(
    self_ptr: *mut ApertureViewfinder,
    location_ptr: *const c_char,
    error_ptr: *mut *mut glib::ffi::GError,
) -> glib::ffi::gboolean {
    let obj: glib::translate::Borrowed<Viewfinder> = unsafe { from_glib_borrow(self_ptr) };
    // Is this ok?
    let location = unsafe { glib::GStr::from_ptr(location_ptr) };

    match obj.take_picture(location) {
        Ok(_) => true.into_glib(),
        Err(err) => {
            let gerr = glib::Error::new(err, &err.to_string());
            unsafe {
                *error_ptr = gerr.as_ptr();
            }
            false.into_glib()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aperture_viewfinder_start_recording(
    self_ptr: *mut ApertureViewfinder,
    location_ptr: *const c_char,
    error_ptr: *mut *mut glib::ffi::GError,
) -> glib::ffi::gboolean {
    let obj: glib::translate::Borrowed<Viewfinder> = unsafe { from_glib_borrow(self_ptr) };
    // Is this ok?
    let location = unsafe { glib::GStr::from_ptr(location_ptr) };

    match obj.start_recording(location) {
        Ok(_) => true.into_glib(),
        Err(err) => {
            let gerr = glib::Error::new(err, &err.to_string());
            unsafe {
                *error_ptr = gerr.as_ptr();
            }
            false.into_glib()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aperture_viewfinder_stop_recording(
    self_ptr: *mut ApertureViewfinder,
    error_ptr: *mut *mut glib::ffi::GError,
) -> glib::ffi::gboolean {
    let obj: glib::translate::Borrowed<Viewfinder> = unsafe { from_glib_borrow(self_ptr) };

    match obj.stop_recording() {
        Ok(_) => true.into_glib(),
        Err(err) => {
            let gerr = glib::Error::new(err, &err.to_string());
            unsafe {
                *error_ptr = gerr.as_ptr();
            }
            false.into_glib()
        }
    }
}
