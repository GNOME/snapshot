use gst::prelude::DeviceExt;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::config;

#[derive(Clone, Copy, Default, Debug)]
enum DeviceType {
    AudioSink,
    AudioSrc,
    VideoSrc,
    #[default]
    Other,
}

mod imp {
    use super::*;

    use std::cell::RefCell;

    use glib::Properties;
    use once_cell::unsync::OnceCell;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::Device)]
    pub struct Device {
        pub nick: RefCell<Option<String>>,
        pub element: OnceCell<gst::Element>,

        #[property(get, set, construct_only)]
        pub inner: OnceCell<gst::Device>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Device {
        const NAME: &'static str = "Device";
        type Type = super::Device;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Device {
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
    pub struct Device(ObjectSubclass<imp::Device>);
}

impl Device {
    pub fn new(inner: &gst::Device) -> Self {
        glib::Object::builder().property("inner", inner).build()
    }

    pub fn element(&self) -> gst::Element {
        self.imp()
            .element
            .get_or_init(glib::clone!(@strong self as obj => move || {
                let inner = obj.inner();

                let element = inner.create_element(None).unwrap();
                element.set_property("client-name", config::APP_ID);
                element.set_property("do-timestamp", true);
                element.set_property("keepalive-time", 1000);
                element.set_property("resend-last", true);

                element
            }))
            .clone()
    }

    fn type_(&self) -> DeviceType {
        match self.inner().device_class().as_str() {
            "Video/Source" => DeviceType::VideoSrc,
            "Audio/Source" => DeviceType::AudioSrc,
            "Audio/Sink" => DeviceType::AudioSink,
            _ => DeviceType::Other,
        }
    }

    pub fn is_camera(&self) -> bool {
        matches!(self.type_(), DeviceType::VideoSrc)
    }

    pub fn display_name(&self) -> glib::GString {
        self.inner().display_name()
    }

    pub fn target_object(&self) -> Option<u64> {
        let element = self.inner();
        if element.has_property("serial", Some(u64::static_type())) {
            Some(element.property::<u64>("serial"))
        } else {
            None
        }
    }
}
