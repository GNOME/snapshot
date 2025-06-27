// SPDX-License-Identifier: GPL-3.0-or-later
use std::cell::RefCell;
use std::collections::HashSet;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::io::RawFd;
use std::sync::Once;

use gst::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::utils;

static STARTED: Once = Once::new();

type ProviderCallback = Box<dyn Fn(&crate::Camera) -> bool + 'static>;

mod imp {
    use std::cell::OnceCell;
    use std::sync::LazyLock;

    use glib::Properties;

    use super::*;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::DeviceProvider)]
    pub struct DeviceProvider {
        pub inner: OnceCell<gst::DeviceProvider>,
        pub cameras: RefCell<Vec<crate::Camera>>,
        pub bus_watch: OnceCell<gst::bus::BusWatchGuard>,

        pub fd: RefCell<Option<OwnedFd>>,

        pub default_cb: OnceCell<ProviderCallback>,

        #[property(get = Self::started)]
        pub started: std::marker::PhantomData<bool>,
    }

    impl DeviceProvider {
        pub fn append(&self, camera: crate::Camera) {
            let pos = self.cameras.borrow().len() as u32;
            self.cameras.borrow_mut().push(camera.clone());
            self.obj().items_changed(pos, 0, 1);
            self.obj().emit_camera_added(&camera);
        }

        fn started(&self) -> bool {
            STARTED.is_completed()
        }

        pub fn has_camera(&self, camera: &crate::Camera) -> bool {
            self.cameras.borrow().iter().any(|c| {
                c.device() == camera.device() || c.target_object() == camera.target_object()
            })
        }

        pub fn remove(&self, device: crate::Camera) {
            let Some(pos) = self
                .cameras
                .borrow()
                .iter()
                .position(|x| x.target_object() == device.target_object())
            else {
                log::error!(
                    "Tried to remove camera with target-object {:?} but it wasn't in the vec?",
                    device.target_object()
                );
                return;
            };
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
                .cloned()
                .and_upcast()
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for DeviceProvider {
        fn constructed(&self) {
            self.parent_constructed();

            crate::ensure_init();

            if let Some(provider) = gst::DeviceProviderFactory::by_name("pipewiredeviceprovider") {
                self.inner.set(provider).unwrap();
            } else {
                log::error!(
                    "Could not create DeviceProviderFactory with name pipewiredeviceprovider"
                );
            }
        }

        fn dispose(&self) {
            let inner = self.inner.get().unwrap();
            if inner.is_started() {
                inner.stop();
            }
            inner.set_property("fd", -1);
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
                vec![
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
    /// A provider for available camera devices.
    ///
    /// It is used to find and monitor cameras that can be used in Aperture. It also handles the
    /// creation of [`Camera`][crate::Camera] objects.
    ///
    /// ## Signals
    ///
    ///
    /// #### `camera-added`
    ///  This signal is emitted after a camera has been added to the device provider.
    ///
    ///
    /// #### `camera-removed`
    ///  This signal is emitted after a camera has been removed from the device provider.
    ///
    /// # Implements
    ///
    /// [`gio::prelude::ListModelExt`][trait@gtk::gio::prelude::ListModelExt], [`glib::ObjectExt`][trait@gtk::glib::ObjectExt]
    pub struct DeviceProvider(ObjectSubclass<imp::DeviceProvider>)
        @implements gio::ListModel;
}

impl DeviceProvider {
    /// Gets the default [`DeviceProvider`][crate::DeviceProvider].
    pub fn instance() -> &'static Self {
        use std::sync::LazyLock;

        use glib::thread_guard::ThreadGuard;

        struct Wrapper(ThreadGuard<crate::DeviceProvider>);
        // SAFETY: We only ever hand out a reference to the contained object on the one
        // thread it was created one so no two threads can access it at the same
        // time.
        unsafe impl Sync for Wrapper {}

        static SINGLETON: LazyLock<Wrapper> = LazyLock::new(|| {
            Wrapper(ThreadGuard::new(
                glib::Object::new::<crate::DeviceProvider>(),
            ))
        });

        SINGLETON.0.get_ref()
    }

    /// Starts the device provider
    ///
    /// Like [`start`] but allows allows to specify a criteria for selecting a
    /// default camera.
    ///
    /// This will be taken into account when the [`Viewfinder`] has to choose a
    /// default camera.
    ///
    /// [`Viewfinder`]: crate::Viewfinder
    /// [`start`]: Self::start()
    pub fn start_with_default<F: Fn(&crate::Camera) -> bool + 'static>(
        &self,
        f: F,
    ) -> Result<(), crate::ProviderError> {
        if STARTED.is_completed() {
            return Ok(());
        }

        STARTED.call_once(|| ());

        let imp = self.imp();

        let Some(provider) = self.imp().inner.get() else {
            return Err(crate::ProviderError::MissingPlugin(
                "pipewiredeviceprovider",
            ));
        };
        provider.start()?;

        let mut seen = HashSet::new();
        let mut cameras = provider
            .devices()
            .iter()
            .filter(|d| is_camera(d))
            .map(crate::Camera::new)
            .filter(|d| !is_ir_camera(d))
            .collect::<Vec<_>>();
        cameras.retain(|item| seen.insert(item.target_object()));
        let n_items = cameras.len() as u32;
        cameras.iter().for_each(|camera| {
            log::debug!(
                "Camera found: {}, target-object: {:?}\nProperties {:#?}\nCaps: {:#?}",
                camera.display_name(),
                camera.target_object(),
                camera.properties(),
                camera.caps(),
            );
        });
        self.imp().cameras.replace(cameras);
        self.items_changed(0, 0, n_items);

        let bus = provider.bus();
        let watch = bus
            .add_watch_local(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move |_, msg| {
                    obj.handle_message(msg);
                    glib::ControlFlow::Continue
                }
            ))
            .expect("Failed to add bus watch");
        imp.bus_watch.set(watch).unwrap();

        let _ = imp.default_cb.set(Box::new(f));

        self.notify_started();

        Ok(())
    }

    /// Starts the device provider represented by `self`.
    ///
    /// This function is idempotent when there are no errors.
    pub fn start(&self) -> Result<(), crate::ProviderError> {
        self.start_with_default(|_| false)
    }

    pub(crate) fn default_camera(&self) -> Option<crate::Camera> {
        let imp = self.imp();
        let cameras = imp.cameras.borrow();
        imp.default_cb
            .get()
            .and_then(|f| cameras.iter().find(|c| f(c)))
            .cloned()
    }

    /// Gets a [`Camera`] object for the given camera index.
    ///
    /// # Returns
    ///
    /// a [`Camera`] at `position`.
    ///
    /// [`Camera`]: crate::Camera
    pub fn camera(&self, position: u32) -> Option<crate::Camera> {
        self.item(position).and_downcast()
    }

    /// Set a valid file description to load and monitor cameras from.
    ///
    /// This file descriptor should point to a valid Pipewire remote where
    /// camera nodes are available. This provider should only provide camera
    /// nodes.
    ///
    /// One way to get a valid descriptor is with the [`org.freedesktop.portal.Camera`](https://flatpak.github.io/xdg-desktop-portal/#gdbus-org.freedesktop.portal.Camera)
    /// XDG portal, using the `OpenPipeWireRemote()` method.
    pub fn set_fd(&self, fd: OwnedFd) -> Result<(), crate::PipewireError> {
        if STARTED.is_completed() {
            return Err(crate::PipewireError::ProvidedStarted);
        }
        let raw_fd = fd.as_raw_fd();
        let provider = self.imp().inner.get().unwrap();
        log::debug!("Starting device provider with file descriptor: {raw_fd}");
        if provider.has_property_with_type("fd", RawFd::static_type()) {
            provider.set_property("fd", raw_fd);
            self.imp()
                .fd
                .replace(Some(fd))
                .inspect(|old_fd| log::debug!("Freeing fd {}", old_fd.as_raw_fd()));
            Ok(())
        } else {
            log::warn!(
                "Pipewire device provider does not have the `fd` property, please update to a version newer than 0.3.64"
            );
            Err(crate::PipewireError::OldVersion)
        }
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
        let imp = self.imp();
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
                if let Some(device) = e
                    .structure()
                    .and_then(|s| s.get::<gst::Device>("device").ok())
                {
                    if !is_camera(&device) {
                        return;
                    }
                    let device = crate::Camera::new(&device);
                    if !imp.has_camera(&device) {
                        // We ignore/filter IR cameras.
                        if is_ir_camera(&device) {
                            log::info!(
                                "IR Camera ignored: {}, target-object: {:?}\nProperties {:#?}\nCaps: {:#?}\nPlease report upstream if this is a false-positive.",
                                device.display_name(),
                                device.target_object(),
                                device.properties(),
                                device.caps(),
                            );
                            return;
                        }
                        log::debug!(
                            "Camera added: {}, target-object: {:?}\nProperties {:#?}\nCaps: {:#?}",
                            device.display_name(),
                            device.target_object(),
                            device.properties(),
                            device.caps(),
                        );
                        imp.append(device);
                    }
                }
            }
            gst::MessageView::DeviceRemoved(e) => {
                if let Some(device) = e
                    .structure()
                    .and_then(|s| s.get::<gst::Device>("device").ok())
                {
                    let n_items = self.n_items();
                    for n in 0..n_items {
                        if let Some(nth_device) = self.camera(n)
                            && device == nth_device.device()
                        {
                            self.imp().remove(nth_device);
                            log::debug!("Camera removed: {}", device.display_name());
                            break;
                        };
                    }
                }
            }
            _ => (),
        }
    }
}

fn is_camera(device: &gst::Device) -> bool {
    device.has_classes("Video/Source")
        && device
            .caps()
            .is_some_and(|c| c.can_intersect(&crate::SUPPORTED_CAPS))
}

fn is_ir_camera(device: &crate::Camera) -> bool {
    device
        .device()
        .caps()
        .as_ref()
        .is_some_and(utils::caps::is_infrared)
        || device.nick().is_some_and(|nick| contains_ir(&nick))
        || contains_ir(&device.display_name())
}

fn contains_ir(s: &str) -> bool {
    s.starts_with("IR ") || s.contains(" IR ") || s.ends_with(" IR")
}
