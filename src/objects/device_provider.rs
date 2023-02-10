// SPDX-License-Identifier: GPL-3.0-or-later
use std::cell::RefCell;
use std::os::unix::io::RawFd;

use gst::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

mod imp {
    use super::*;

    use glib::Properties;
    use once_cell::unsync::OnceCell;

    use std::cell::Cell;

    #[derive(Debug, Properties)]
    #[properties(wrapper_type = super::DeviceProvider)]
    pub struct DeviceProvider {
        pub provider: OnceCell<gst::DeviceProvider>,
        pub cameras: RefCell<Vec<crate::Device>>,

        #[property(get, set, construct_only, default_value = -1)]
        fd: Cell<RawFd>,
    }

    impl Default for DeviceProvider {
        fn default() -> Self {
            Self {
                provider: Default::default(),
                cameras: Default::default(),
                fd: Cell::new(-1),
            }
        }
    }

    impl DeviceProvider {
        pub fn append(&self, device: crate::Device) {
            let pos = self.cameras.borrow().len() as u32;
            self.cameras.borrow_mut().push(device);
            self.obj().items_changed(pos, 0, 1);
        }

        pub fn remove(&self, device: crate::Device) {
            let guard = self.cameras.borrow();
            let Some((pos, _)) =  guard.iter().enumerate().find(|(_idx, x)| x.target_object() == device.target_object()) else {
                log::error!("Tried to remove camera with target-object {:?} but it wasn't in the vec?", device.target_object());
                return;
            };
            drop(guard);
            self.cameras.borrow_mut().remove(pos);
            self.obj().items_changed(pos as u32, 1, 0);
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceProvider {
        const NAME: &'static str = "DeviceProvider";
        type Type = super::DeviceProvider;
        type ParentType = glib::Object;
        type Interfaces = (gio::ListModel,);
    }

    impl ListModelImpl for DeviceProvider {
        fn item_type(&self) -> glib::Type {
            crate::Device::static_type()
        }
        fn n_items(&self) -> u32 {
            self.cameras.borrow().len() as u32
        }
        fn item(&self, position: u32) -> Option<glib::Object> {
            self.cameras
                .borrow()
                .get(position as usize)
                .map(|o| o.clone().upcast::<glib::Object>())
        }
    }

    impl ObjectImpl for DeviceProvider {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            Self::derived_set_property(self, id, value, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            let provider = gst::DeviceProviderFactory::by_name("pipewiredeviceprovider").unwrap();
            let fd = obj.fd();
            if fd > -1 {
                log::debug!("Starting device provider with file descriptor: {fd}");
                if provider.has_property("fd", Some(RawFd::static_type())) {
                    provider.set_property("fd", &fd);
                }
            }
            self.provider.set(provider).unwrap();
        }

        fn dispose(&self) {
            let inner = self.provider.get().unwrap();
            if inner.is_started() {
                inner.stop();
            }
            let bus = inner.bus();
            let _ = bus.remove_watch();
            let raw_fd = self.fd.replace(-1);
            if raw_fd > -1 {
                unsafe {
                    // FIXME Replace with a OwnedFd once
                    // https://github.com/bilelmoussaoui/ashpd/pull/104 is merged.
                    libc::close(raw_fd);
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct DeviceProvider(ObjectSubclass<imp::DeviceProvider>)
        @implements gio::ListModel;
}

impl DeviceProvider {
    /// Creates a device provider, if a file descriptor coming for the Camera
    /// portal is passed, this will only list camera devices.
    pub fn new(fd: Option<RawFd>) -> Self {
        glib::Object::builder()
            .property("fd", fd.unwrap_or(-1))
            .build()
    }

    pub fn start(&self) -> anyhow::Result<()> {
        let provider = self.imp().provider.get().unwrap();
        self.imp().provider.get().unwrap().start()?;

        let bus = provider.bus();
        bus.add_watch_local(
                glib::clone!(@weak self as obj => @default-return glib::Continue(false),
                move |_, msg| {
                    match msg.view() {
                        gst::MessageView::Error(err) => {
                            log::error!(
                                "Error from {:?}: {} ({:?})",
                                err.src().map(|s| s.path_string()),
                                err.error(),
                                err.debug()
                            );
                        }
                        gst::MessageView::DeviceAdded(e) => {
                            if let Some(s) = e.structure() {
                                if let Ok(device) = s.get::<gst::Device>("device") {
                                    if "Video/Source" == device.device_class().as_str() {
                                        log::debug!("Camera added: {}, target-object {}", device.display_name(), device.property::<u64>("serial"));
                                        let device = crate::Device::new(&device);
                                        obj.imp().append(device);
                                    };
                                }
                            }
                        }
                        gst::MessageView::DeviceRemoved(e) => {
                            if let Some(s) = e.structure() {
                                if let Ok(device) = s.get::<gst::Device>("device") {
                                    if "Video/Source" == device.device_class().as_str() {
                                        log::debug!("Camera removed: {}", device.display_name());
                                        let device = crate::Device::new(&device);
                                        obj.imp().remove(device);
                                    };
                                }
                            }
                        }
                        gst::MessageView::DeviceChanged(e) => {
                            if let Some(s) = e.structure() {
                                if let Ok(device) = s.get::<gst::Device>("device") {
                                    if "Video/Source" == device.device_class().as_str() {
                                        // TODO Implement
                                        log::debug!("Camera changed: {}, target-object {}", device.display_name(), device.property::<u64>("serial"));
                                    };
                                }
                            }
                        },
                        _ => (),
                    }
                    glib::Continue(true)
                }))
               .expect("Failed to add bus watch");

        Ok(())
    }

    pub fn camera(&self, position: u32) -> Option<crate::Device> {
        self.item(position).and_downcast()
    }
}
