// SPDX-License-Identifier: GPL-3.0-or-later
use std::cell::RefCell;
use std::os::unix::io::RawFd;
use std::sync::Once;

use gst::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

mod imp {
    use super::*;

    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;

    #[derive(Debug, Default)]
    pub struct DeviceProvider {
        pub inner: OnceCell<gst::DeviceProvider>,
        pub cameras: RefCell<Vec<crate::Camera>>,

        pub fd: RefCell<Option<RawFd>>,
    }

    impl DeviceProvider {
        pub fn append(&self, device: crate::Camera) {
            let mut guard = self.cameras.borrow_mut();
            let pos = guard.len() as u32;
            if !guard.contains(&device) {
                guard.push(device.clone());
                drop(guard);
                self.obj().items_changed(pos, 0, 1);
                self.obj().emit_camera_added(&device);
            }
        }

        pub fn remove(&self, device: crate::Camera) {
            let guard = self.cameras.borrow();
            let Some((pos, _)) =  guard.iter().enumerate().find(|(_idx, x)| x.target_object() == device.target_object()) else {
                log::error!("Tried to remove camera with target-object {:?} but it wasn't in the vec?", device.target_object());
                return;
            };
            drop(guard);
            self.cameras.borrow_mut().remove(pos);
            self.obj().items_changed(pos as u32, 1, 0);
            self.obj().emit_camera_removed(&device);
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceProvider {
        const NAME: &'static str = "ApertureDeviceProvider";
        type Type = super::DeviceProvider;
        type Interfaces = (gio::ListModel,);
    }

    impl ListModelImpl for DeviceProvider {
        fn item_type(&self) -> glib::Type {
            crate::Camera::static_type()
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

            crate::ensure_init();

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

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![
                    // These are emited whenever the saving process finishes,
                    // successful or not.
                    glib::subclass::Signal::builder("camera-added")
                        .param_types([crate::Camera::static_type()])
                        .build(),
                    glib::subclass::Signal::builder("camera-removed")
                        .param_types([crate::Camera::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }
}

glib::wrapper! {
    pub struct DeviceProvider(ObjectSubclass<imp::DeviceProvider>)
        @implements gio::ListModel;
}

impl DeviceProvider {
    /// Gets the default `DeviceProvider`.
    pub fn instance() -> &'static Self {
        use glib::thread_guard::ThreadGuard;
        use once_cell::sync::Lazy;

        struct Wrapper(ThreadGuard<crate::DeviceProvider>);
        // SAFETY: We only ever hand out a reference to the contained object on the one thread
        // it was created one so no two threads can access it at the same time.
        unsafe impl Sync for Wrapper {}

        static SINGLETON: Lazy<Wrapper> = Lazy::new(|| {
            Wrapper(ThreadGuard::new(
                glib::Object::new::<crate::DeviceProvider>(),
            ))
        });

        SINGLETON.0.get_ref()
    }

    /// Starts the device provider.
    ///
    /// This function is idempotent when there are no errors.
    ///
    /// [`crate::Viewfinder`] automatically calls this function.
    pub fn start(&self) -> anyhow::Result<()> {
        static STARTED: Once = Once::new();

        if STARTED.is_completed() {
            return Ok(());
        }

        let provider = self.imp().inner.get().unwrap();
        provider.start()?;

        STARTED.call_once(glib::clone!(@weak self as obj, @weak provider => move || {
            let bus = provider.bus();
            bus.add_watch_local(
                glib::clone!(@weak obj => @default-return glib::Continue(false),
                    move |_, msg| {
                        obj.handle_message(msg);
                        glib::Continue(true)
                    })
            ).expect("Failed to add bus watch");
        }));

        Ok(())
    }

    /// Gets a [`Camera`] object for the given camera index.
    pub fn camera(&self, position: u32) -> Option<crate::Camera> {
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

    pub fn connect_camera_added<F: Fn(&Self, &crate::Camera) + 'static>(&self, f: F) {
        self.connect_closure(
            "camera-added",
            false,
            glib::closure_local!(|obj, camera| {
                f(obj, camera);
            }),
        );
    }

    pub fn connect_camera_removed<F: Fn(&Self, &crate::Camera) + 'static>(&self, f: F) {
        self.connect_closure(
            "camera-removed",
            false,
            glib::closure_local!(|obj, camera| {
                f(obj, camera);
            }),
        );
    }

    fn emit_camera_added(&self, camera: &crate::Camera) {
        self.emit_by_name::<()>("camera-added", &[&camera]);
    }

    fn emit_camera_removed(&self, camera: &crate::Camera) {
        self.emit_by_name::<()>("camera-removed", &[&camera]);
    }

    fn handle_message(&self, msg: &gst::Message) {
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
                            let device = crate::Camera::new(&device);
                            log::debug!(
                                "Camera added: {}, target-object: {:?}\nProperties:\n{:#?}\nCaps:\n{:#?}",
                                device.display_name(),
                                device.target_object(),
                                device.properties(),
                                device.caps(),
                            );
                            self.imp().append(device);
                        };
                    }
                }
            }
            gst::MessageView::DeviceRemoved(e) => {
                if let Some(s) = e.structure() {
                    if let Ok(device) = s.get::<gst::Device>("device") {
                        if "Video/Source" == device.device_class().as_str() {
                            let n_items = self.n_items();
                            for n in 0..n_items {
                                if let Some(nth_device) = self.camera(n) {
                                    if device == nth_device.device() {
                                        self.imp().remove(nth_device);
                                        log::debug!("Camera removed: {}", device.display_name());
                                        break;
                                    }
                                };
                            }
                        };
                    }
                }
            }
            _ => (),
        }
    }
}
