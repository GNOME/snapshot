// SPDX-License-Identifier: GPL-3.0-or-later
use std::cell::RefCell;
use std::os::unix::io::RawFd;

use gst::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

mod imp {
    use super::*;

    use once_cell::unsync::OnceCell;

    #[derive(Debug, Default)]
    pub struct DeviceProvider {
        pub inner: OnceCell<gst::DeviceProvider>,
        pub cameras: RefCell<Vec<crate::Device>>,

        pub fd: RefCell<Option<RawFd>>,
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
        fn constructed(&self) {
            self.parent_constructed();

            let provider = gst::DeviceProviderFactory::by_name("pipewiredeviceprovider").unwrap();
            self.inner.set(provider).unwrap();
        }

        fn dispose(&self) {
            let inner = self.inner.get().unwrap();
            if inner.is_started() {
                inner.stop();
            }
            let bus = inner.bus();
            let _ = bus.remove_watch();
            if let Some(raw_fd) = self.fd.take() {
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

impl Default for DeviceProvider {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl DeviceProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&self) -> anyhow::Result<()> {
        let provider = self.imp().inner.get().unwrap();
        provider.start()?;

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

    /// A file descriptor coming for the Camera portal. Such a provider can only
    /// provide cameras.
    pub fn set_fd(&self, fd: RawFd) -> anyhow::Result<()> {
        let provider = self.imp().inner.get().unwrap();
        log::debug!("Starting device provider with file descriptor: {fd}");
        if provider.has_property("fd", Some(RawFd::static_type())) {
            provider.set_property("fd", &fd);
            self.imp().fd.replace(Some(fd));
        } else {
            anyhow::bail!("Pipewire device provider does not have the `fd` property, please update to a version newer than 0.3.64");
        }

        Ok(())
    }
}
