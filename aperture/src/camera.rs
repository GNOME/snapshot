// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::HashMap;

use gst::prelude::DeviceExt;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::utils;

mod imp {
    use std::cell::OnceCell;

    use glib::Properties;

    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::Camera)]
    pub struct Camera {
        #[property(get, set, construct_only)]
        device: OnceCell<gst::Device>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Camera {
        const NAME: &'static str = "ApertureCamera";
        type Type = super::Camera;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Camera {}
}

glib::wrapper! {
    /// A representation of a camera plugged into a device.
    ///
    /// It is used to query information about a camera or change its parameters. Camera objects
    /// should not be created by a user, and should only be created via a [`DeviceProvider`][crate::DeviceProvider].
    ///
    /// ## Properties
    ///
    ///
    /// #### `device`
    ///  The [`gst::Device`][gst::Device] to which this camera represents.
    ///
    /// Readable | Writeable
    ///
    /// # Implements
    ///
    /// [`glib::ObjectExt`][trait@gtk::glib::ObjectExt]
    pub struct Camera(ObjectSubclass<imp::Camera>);
}

impl Camera {
    /// Gets the display name of the camera represented by `self`.
    ///
    /// # Returns
    ///
    /// the display name.
    pub fn display_name(&self) -> glib::GString {
        self.device().display_name()
    }

    /// Gets the user-set nickname of the camera represented by `self`.
    ///
    /// # Returns
    ///
    /// the display name if set.
    pub fn nick(&self) -> Option<String> {
        self.device().properties().and_then(|properties| {
            properties
                .value("node.nick")
                .ok()
                .and_then(|value| value.get::<String>().ok())
        })
    }

    /// Gets all the available properties for the camera represented by `self`.
    ///
    /// # Returns
    ///
    /// a [`HashMap`][std::collections::HashMap], with the property name as the
    /// key and a [`GValue`][gtk::glib::Value] as the value.
    pub fn properties(&self) -> HashMap<String, glib::SendValue> {
        self.device()
            .properties()
            .map(|s| {
                s.iter()
                    .map(|(key, val)| (key.to_string(), val.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Gets the supported [`caps`](https://gstreamer.freedesktop.org/documentation/additional/design/caps.html)
    /// of the camera represented by `self`.
    ///
    /// # Returns
    ///
    /// the available caps if available.
    pub fn caps(&self) -> Option<gst::Caps> {
        self.device().caps().as_ref().map(utils::caps::limit_fps)
    }

    /// Gets the location of the camera represented by `self`.
    /// This function requires `libcamera` to be available.
    ///
    /// # Returns
    ///
    /// the [`CameraLocation`][crate::CameraLocation].
    pub fn location(&self) -> crate::CameraLocation {
        self.device()
            .properties()
            .and_then(|properties| {
                properties
                    .value("api.libcamera.location")
                    .ok()
                    .and_then(|value| value.get::<&str>().ok())
                    .map(|loc| loc.into())
            })
            .unwrap_or_default()
    }

    /// Gets the `serial` of the device
    ///
    /// For newer pipewire versions this corresponds to the `target-object` of
    /// the element and for older versions this corresponds to the `path` of the
    /// device.
    pub(crate) fn target_object(&self) -> Option<u64> {
        let device = self.device();
        if device.has_property_with_type("serial", u64::static_type()) {
            Some(device.property::<u64>("serial"))
        } else {
            None
        }
    }

    pub(crate) fn new(device: &gst::Device) -> Self {
        glib::Object::builder().property("device", device).build()
    }

    pub(crate) fn reconfigure(&self, element: &gst::Element) -> Result<(), glib::BoolError> {
        self.device().reconfigure_element(element)
    }

    pub(crate) fn create_element(&self) -> Result<gst::Element, glib::BoolError> {
        let element = self.device().create_element(None)?;
        element.set_property("client-name", crate::APP_ID.get().unwrap());
        Ok(element)
    }

    pub(crate) fn best_caps(&self) -> gst::Caps {
        let caps = self
            .caps()
            .unwrap_or_else(|| gst::Caps::builder("video/x-raw").build());
        let highest_res_caps = filter_caps(caps);
        log::debug!("Using caps: {highest_res_caps:#?}");

        highest_res_caps
    }
}

// For each resolution and format we only keep the highest resolution.
fn filter_caps(caps: gst::Caps) -> gst::Caps {
    let mut best_caps = gst::Caps::new_empty();
    caps.iter().for_each(|s| {
        if let Some(framerate) = framerate_from_structure(s) {
            let best = utils::caps::best_resolution_for_fps(&caps, framerate);
            best_caps.merge(best);
        }
    });

    best_caps.merge(caps);
    best_caps
}

fn framerate_from_structure(structure: &gst::StructureRef) -> Option<gst::Fraction> {
    // TODO Handle gst::List and gst::Array
    if let Ok(framerate) = structure.get::<gst::Fraction>("framerate") {
        Some(framerate)
    } else if let Ok(range) = structure.get::<gst::FractionRange>("framerate") {
        Some(range.max())
    } else if let Ok(array) = structure.get::<gst::Array>("framerate") {
        array
            .iter()
            .filter_map(|s| s.get::<gst::Fraction>().ok())
            .filter(|frac| frac <= &gst::Fraction::new(crate::MAXIMUM_RATE, 1))
            .max()
    } else if let Ok(array) = structure.get::<gst::List>("framerate") {
        array
            .iter()
            .filter_map(|s| s.get::<gst::Fraction>().ok())
            .filter(|frac| frac <= &gst::Fraction::new(crate::MAXIMUM_RATE, 1))
            .max()
    } else {
        None
    }
}
