// SPDX-License-Identifier: GPL-3.0-or-later
use gst::prelude::DeviceExt;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::collections::HashMap;

mod imp {
    use super::*;

    use glib::Properties;
    use once_cell::unsync::OnceCell;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::Camera)]
    pub struct Camera {
        #[property(get, set)]
        device: OnceCell<gst::Device>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Camera {
        const NAME: &'static str = "ApertureCamera";
        type Type = super::Camera;
    }

    impl ObjectImpl for Camera {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            Self::derived_set_property(self, id, value, pspec)
        }
    }
}

glib::wrapper! {
    pub struct Camera(ObjectSubclass<imp::Camera>);
}

impl Camera {
    /// Gets the display name of the camera
    pub fn display_name(&self) -> glib::GString {
        self.device().display_name()
    }

    /// Gets the display name of the camera
    pub fn nick(&self) -> Option<String> {
        self.device().properties().and_then(|properties| {
            properties
                .value("node.nick")
                .ok()
                .and_then(|value| value.get::<String>().ok())
        })
    }

    /// Gets the properties of the device
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

    /// Gets the supported caps of the device
    pub fn caps(&self) -> Option<gst::Caps> {
        self.device().caps()
    }

    /// Gets the camera location
    ///
    /// Requires libcamera to work.
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
    ) -> Option<(gst::Element, gst::Element)> {
        let device = self.device();
        let Some(previous) = previous else {
            return create_element(&device);
        };
        match device.reconfigure_element(previous) {
            Ok(_) => None,
            Err(_) => create_element(&device),
        }
    }
}

fn create_element(device: &gst::Device) -> Option<(gst::Element, gst::Element)> {
    use gst::prelude::*;

    let bin = gst::Bin::new(None);

    let device_src = device.create_element(None).ok()?;
    device_src.set_property("client-name", crate::APP_ID.get().unwrap());

    let videoflip = gst::ElementFactory::make("videoflip")
        .property_from_str("video-direction", "auto")
        .build()
        .unwrap();

    bin.add_many(&[&device_src, &videoflip]).unwrap();
    device_src.link(&videoflip).unwrap();

    let pad = videoflip.static_pad("src").unwrap();
    let ghost_pad = gst::GhostPad::with_target(Some("src"), &pad).unwrap();
    ghost_pad.set_active(true).unwrap();

    bin.add_pad(&ghost_pad).unwrap();

    let wrapper = gst::ElementFactory::make("wrappercamerabinsrc")
        .property("video-source", &bin)
        .build()
        .unwrap();

    Some((wrapper, device_src))
}
