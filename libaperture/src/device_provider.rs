use gtk::glib;
use gtk::glib::translate::*;
use gtk::subclass::prelude::*;

use crate::ApertureCamera;
use aperture::DeviceProvider;

pub type ApertureDeviceProvider = <aperture::DeviceProviderInner as ObjectSubclass>::Instance;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aperture_device_provider_get_default() -> *mut ApertureDeviceProvider {
    DeviceProvider::instance().to_glib_none().0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aperture_device_provider_start(
    self_ptr: *mut ApertureDeviceProvider,
) -> glib::ffi::gboolean {
    let obj: glib::translate::Borrowed<DeviceProvider> = unsafe { from_glib_borrow(self_ptr) };
    obj.start().is_ok().into_glib()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aperture_device_provider_get_camera(
    self_ptr: *mut ApertureDeviceProvider,
    camera: u32,
) -> *mut ApertureCamera {
    let obj: glib::translate::Borrowed<DeviceProvider> = unsafe { from_glib_borrow(self_ptr) };
    obj.camera(camera).to_glib_full()
}
