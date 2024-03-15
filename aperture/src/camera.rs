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
    pub fn properties(&self) -> HashMap<&'static str, glib::SendValue> {
        self.device()
            .properties()
            .map(|s| {
                s.iter()
                    .map(|(key, val)| (key.as_ref(), val.clone()))
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
        self.device().caps()
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
        if device.has_property("serial", Some(u64::static_type())) {
            Some(device.property::<u64>("serial"))
        } else {
            None
        }
    }

    pub(crate) fn new(device: &gst::Device) -> Self {
        glib::Object::builder().property("device", device).build()
    }

    pub(crate) fn source_element(
        &self,
        previous: Option<&gst::Element>,
    ) -> Result<Option<(gst::Element, gst::Element)>, glib::BoolError> {
        let device = self.device();
        let Some(previous) = previous else {
            return create_element(&device);
        };
        match device.reconfigure_element(previous) {
            Ok(_) => Ok(None),
            Err(_) => create_element(&device),
        }
    }
}

fn caps(device: &gst::Device) -> gst::Caps {
    let device_caps = device
        .caps()
        .unwrap_or_else(|| gst_video::VideoCapsBuilder::for_encoding("video/x-raw").build());
    utils::caps::limit_fps(&device_caps)
}

// For each resolution and format we only keep the highest resolution.
fn filter_caps(caps: &gst::Caps) -> gst::Caps {
    let mut best_caps = gst::Caps::new_empty();
    caps.iter().for_each(|s| {
        if let Some(framerate) = framerate_from_structure(s) {
            let best = utils::caps::best_resolution_for_fps(caps, framerate);
            best_caps.merge(best);
        }
    });

    caps.intersect_with_mode(&best_caps, gst::CapsIntersectMode::First)
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
            .filter(|frac| frac < &gst::Fraction::new(crate::MAXIMUM_RATE, 1))
            .max()
    } else if let Ok(array) = structure.get::<gst::List>("framerate") {
        array
            .iter()
            .filter_map(|s| s.get::<gst::Fraction>().ok())
            .filter(|frac| frac < &gst::Fraction::new(crate::MAXIMUM_RATE, 1))
            .max()
    } else {
        None
    }
}

fn create_element(
    device: &gst::Device,
) -> Result<Option<(gst::Element, gst::Element)>, glib::BoolError> {
    use gst::prelude::*;

    let bin = gst::Bin::new();

    let device_src = device.create_element(None)?;
    device_src.set_property("client-name", crate::APP_ID.get().unwrap());

    let caps = caps(device);
    log::debug!("Found caps: {caps}");
    let highest_res_caps = filter_caps(&caps);
    log::debug!("Using caps: {highest_res_caps}");
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .property("caps", &highest_res_caps)
        .build()?;
    let decodebin3 = gst::ElementFactory::make("decodebin3").build()?;

    let videoflip = gst::ElementFactory::make("videoflip")
        .property_from_str("video-direction", "auto")
        .build()?;

    bin.add_many([&device_src, &capsfilter, &decodebin3, &videoflip])?;
    gst::Element::link_many([&device_src, &capsfilter, &decodebin3])?;

    decodebin3.connect_pad_added(glib::clone!(@weak videoflip => move |_, pad| {
        if pad.stream().is_some_and(|stream| matches!(stream.stream_type(), gst::StreamType::VIDEO)) {
            pad.link(&videoflip.static_pad("sink").unwrap())
               .expect("Failed to link decodebin3:video_%u pad with videoflip:sink");
        }
    }));

    let pad = videoflip.static_pad("src").unwrap();
    let ghost_pad = gst::GhostPad::with_target(&pad)?;
    ghost_pad.set_active(true)?;

    bin.add_pad(&ghost_pad)?;

    let wrapper = gst::ElementFactory::make("wrappercamerabinsrc")
        .property("video-source", &bin)
        .build()
        .expect("Missing GStreamer Bad Plug-ins");

    Ok(Some((wrapper, device_src)))
}
