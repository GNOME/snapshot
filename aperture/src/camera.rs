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
        #[property(get, set, construct_only)]
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
    /// a [`HashMap`][std::collections::HashMap], with the property name as the key
    /// and a [`GValue`][gtk::glib::Value] as the value.
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

    let mut caps = gst_video::VideoCapsBuilder::new().any_features().build();
    caps.merge(gst::Caps::builder("image/jpeg").build());

    let capsfilter = gst::ElementFactory::make("capsfilter")
        .property("caps", caps)
        .build()
        .unwrap();
    let decodebin3 = gst::ElementFactory::make("decodebin3")
        .property(
            "caps",
            gst_video::VideoCapsBuilder::new().any_features().build(),
        )
        .build()
        .unwrap();

    let videoflip = gst::ElementFactory::make("videoflip")
        .property_from_str("video-direction", "auto")
        .build()
        .unwrap();

    bin.add_many(&[&device_src, &capsfilter, &decodebin3, &videoflip])
        .unwrap();
    gst::Element::link_many(&[&device_src, &capsfilter, &decodebin3]).unwrap();

    decodebin3.connect_pad_added(glib::clone!(@weak videoflip => move |_, pad| {
        // TODO Use is_some_and once stabilized
        if pad.stream().map(|stream| matches!(stream.stream_type(), gst::StreamType::VIDEO)).unwrap_or_default() {
            pad.link(&videoflip.static_pad("sink").unwrap())
               .expect("Failed to link decodebin3:video_%u pad with videoflip:sink");
        }
    }));

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
