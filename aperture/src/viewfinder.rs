// SPDX-License-Identifier: GPL-3.0-or-later
use std::path::Path;
use std::path::PathBuf;

use gst::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene};
use once_cell::sync::Lazy;

use crate::ViewfinderState;

const BARCODE_TIMEOUT: u32 = 1;
const PROVIDER_TIMEOUT: u64 = 2;

#[derive(Debug)]
enum StateChangeState {
    Equal,
    Differ,
    Error,
    NotDone,
}

mod imp {
    use std::cell::Cell;
    use std::cell::OnceCell;
    use std::cell::RefCell;

    use glib::Properties;

    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::Viewfinder)]
    pub struct Viewfinder {
        #[property(get, explicit_notify, builder(Default::default()))]
        state: Cell<ViewfinderState>,
        #[property(get = Self::detect_codes, set = Self::set_detect_codes, explicit_notify)]
        detect_codes: Cell<bool>,
        #[property(get, set = Self::set_camera, nullable, explicit_notify)]
        camera: RefCell<Option<crate::Camera>>,
        #[property(get = Self::is_recording, name = "is-recording", type = bool)]
        pub is_recording_video: RefCell<Option<PathBuf>>,

        pub zbar_branch: RefCell<Option<gst::Element>>,
        pub devices: OnceCell<crate::DeviceProvider>,
        pub camera_src: RefCell<Option<gst::Element>>,
        pub camerabin: OnceCell<gst::Element>,
        pub camera_element: OnceCell<gst::Element>,
        pub capsfilter: OnceCell<gst::Element>,
        pub sink_paintable: OnceCell<gst::Element>,
        pub tee: OnceCell<crate::PipelineTee>,
        pub bus_watch: OnceCell<gst::bus::BusWatchGuard>,

        pub is_stopping_recording: Cell<bool>,
        pub is_taking_picture: Cell<bool>,
        pub is_front_camera: Cell<bool>,

        pub timeout_handler: RefCell<Option<glib::SourceId>>,

        pub picture: gtk::Picture,
    }

    impl Viewfinder {
        pub fn camerabin(&self) -> &gst::Element {
            self.camerabin.get().unwrap()
        }

        pub(crate) fn set_state(&self, state: ViewfinderState) {
            if state != self.state.replace(state) {
                self.obj().notify_state();
            }
        }

        fn is_recording(&self) -> bool {
            self.is_recording_video.borrow().is_some()
        }

        fn detect_codes(&self) -> bool {
            self.zbar_branch.borrow().is_some()
        }

        fn set_detect_codes(&self, value: bool) {
            if value == self.detect_codes.replace(value) {
                return;
            }

            let tee = self.tee.get().unwrap();
            if value {
                match create_zbar_bin() {
                    Ok(zbar_branch) => {
                        tee.add_branch(&zbar_branch);
                        self.zbar_branch.replace(Some(zbar_branch));
                    }
                    Err(err) => {
                        log::error!("Could not create zbar element: {err}");
                    }
                }
            } else if let Some(zbar_branch) = self.zbar_branch.take() {
                tee.remove_branch(&zbar_branch);
            }

            self.obj().notify_detect_codes();
        }

        /// Sets the camera that the `ApertureViewfinder` will use.
        fn set_camera(&self, camera: Option<crate::Camera>) {
            let obj = self.obj();

            if !matches!(obj.state(), ViewfinderState::Ready | ViewfinderState::Error) {
                log::error!("Could not set camera, the viewfinder is not ready");
                return;
            }

            if self.is_taking_picture.get() {
                log::error!("Could not set camera, where are taking a picture");
                return;
            }

            if self.is_recording_video.borrow().is_some() {
                log::error!("Could not set camera, there is a recording in progress");
                return;
            }

            if camera == self.camera.replace(camera.clone()) {
                return;
            }

            // We reset to READY if we landed on the ERROR state on the previous
            // camera.
            if matches!(obj.state(), ViewfinderState::Error) {
                if self
                    .devices
                    .get()
                    .and_then(|devices| devices.camera(0))
                    .is_some()
                {
                    self.set_state(ViewfinderState::Ready);
                } else {
                    self.set_state(ViewfinderState::NoCameras);
                }
            }

            // The current state is PAUSED if there was an error on the previous camera.
            if obj.is_realized()
                && matches!(
                    self.camerabin().current_state(),
                    gst::State::Playing | gst::State::Paused
                )
            {
                obj.stop_stream();
            }

            if let Some(camera) = camera {
                if let Err(err) = obj.setup_camera_element(&camera) {
                    log::error!("Could not reconfigure camera element: {err}");
                    self.set_state(ViewfinderState::Error);
                }
            }

            if obj.is_realized() && matches!(obj.state(), ViewfinderState::Ready) {
                obj.start_stream();
            }

            obj.notify_camera();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Viewfinder {
        const NAME: &'static str = "ApertureViewfinder";
        type Type = super::Viewfinder;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for Viewfinder {
        fn constructed(&self) {
            self.parent_constructed();

            crate::ensure_init();

            let obj = self.obj();

            let camerabin = gst::ElementFactory::make("camerabin")
                .property("location", None::<&str>)
                .build()
                .expect("Missing GStreamer Bad Plug-ins");
            self.camerabin.set(camerabin.clone()).unwrap();

            let bus = self.camerabin().bus().unwrap();
            let watch = bus.add_watch_local(
                glib::clone!(@weak obj => @default-return glib::ControlFlow::Break, move |_, msg| {
                    obj.on_bus_message(msg);
                    glib::ControlFlow::Continue
                }),
            )
            .unwrap();
            self.bus_watch.set(watch).unwrap();

            let tee = crate::PipelineTee::new();

            let paintablesink = gst::ElementFactory::make("gtk4paintablesink")
                .build()
                .unwrap();

            let paintable = paintablesink.property::<gdk::Paintable>("paintable");
            let is_gl_supported = paintable
                .property::<Option<gdk::GLContext>>("gl-context")
                .is_some();
            let sink = if is_gl_supported {
                gst::ElementFactory::make("glsinkbin")
                    .property("sink", &paintablesink)
                    .build()
                    .expect("Missing GStreamer Base Plug-ins")
            } else {
                let bin = gst::Bin::default();
                let convert = gst::ElementFactory::make("videoconvert")
                    .build()
                    .expect("Missing GStreamer Base Plug-ins");

                bin.add(&convert).unwrap();
                bin.add(&paintablesink).unwrap();
                convert.link(&paintablesink).unwrap();

                bin.add_pad(
                    &gst::GhostPad::with_target(&convert.static_pad("sink").unwrap()).unwrap(),
                )
                .unwrap();

                bin.upcast()
            };

            tee.add_branch(&sink);
            camerabin.set_property("viewfinder-sink", &tee);

            self.sink_paintable.set(paintablesink).unwrap();

            self.picture
                .set_accessible_role(gtk::AccessibleRole::Presentation);
            self.picture.set_hexpand(true);
            self.picture.set_vexpand(true);
            self.picture.set_parent(&*obj);
            self.picture.set_paintable(Some(&paintable));

            self.tee.set(tee).unwrap();

            let devices = crate::DeviceProvider::instance();

            self.devices.set(devices.clone()).unwrap();

            if devices.started() {
                obj.init();
            } else {
                devices.connect_started_notify(glib::clone!(@weak obj => move |_| {
                    obj.init();
                }));
            }

            devices.connect_camera_added(glib::clone!(@weak obj => move |_, camera| {
                if matches!(obj.state(), ViewfinderState::NoCameras | ViewfinderState::Loading | ViewfinderState::Error) {
                    obj.imp().set_state(ViewfinderState::Ready);
                    obj.set_camera(Some(camera.clone()));
                }
            }));

            devices.connect_camera_removed(glib::clone!(@weak obj => move |devices, camera| {
                let imp = obj.imp();
                if Some(camera) == imp.camera.borrow().as_ref() {
                    obj.cancel_current_operation();

                    let next_camera = devices.camera(0);
                    let is_none = next_camera.is_none();
                    obj.set_camera(next_camera);
                    if is_none {
                        obj.imp().set_state(ViewfinderState::NoCameras);
                    }
                }
            }));

            log::debug!("Setup recording");
            obj.setup_recording();
        }

        fn dispose(&self) {
            if self.is_recording_video.borrow().is_some() {
                if let Err(err) = self.obj().stop_recording() {
                    log::error!("Could not stop recording: {err}");
                }
            }
            if let Err(err) = self.camerabin().set_state(gst::State::Null) {
                log::error!("Could not stop camerabin: {err}");
            }

            self.picture.unparent();
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![
                    // These are emitted whenever the saving process finishes,
                    // successful or not.
                    glib::subclass::Signal::builder("picture-done")
                        .param_types([Option::<gio::File>::static_type()])
                        .build(),
                    glib::subclass::Signal::builder("recording-done")
                        .param_types([Option::<gio::File>::static_type()])
                        .build(),
                    glib::subclass::Signal::builder("code-detected")
                        .param_types([crate::CodeType::static_type(), String::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for Viewfinder {
        fn realize(&self) {
            self.parent_realize();

            if matches!(self.obj().state(), ViewfinderState::Ready) {
                self.obj().start_stream();
            }
        }

        fn unrealize(&self) {
            self.obj().stop_stream();

            self.parent_unrealize();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let w = self.obj().width() as f32;
            let h = self.obj().height() as f32;

            // Make the background black
            let rect = graphene::Rect::new(0.0, 0.0, w, h);
            snapshot.append_color(&gdk::RGBA::BLACK, &rect);

            // This is the composition of translate (-w / 2.0, 0.0), map x to
            // -x, and translate (w / 2.0 , 0.0). Note that gsk matrices are
            // transposed (they act on row vectors).
            if self.is_front_camera.get() {
                #[rustfmt::skip]
                let flip_matrix = graphene::Matrix::from_float([
                    -1.0,  0.0,  0.0,  0.0,
                     0.0,  1.0,  0.0,  0.0,
                     0.0,  0.0,  1.0,  0.0,
                       w,  0.0,  0.0,  1.0,
                ]);
                snapshot.save();
                snapshot.transform_matrix(&flip_matrix);
                self.parent_snapshot(snapshot);
                snapshot.restore();
            } else {
                self.parent_snapshot(snapshot);
            }
        }
    }
}

glib::wrapper! {
    /// A GTK widget for displaying a camera feed and taking pictures and videos from it.
    ///
    /// The viewfinder is the main widget in Aperture, and is responsible for displaying a camera
    /// feed in your UI; along with using that camera feed to do useful tasks, like take pictures,
    /// record video, and detect barcodes.
    ///
    /// The viewfinder does not contain any camera controls, these must be implemented yourself.
    ///
    ///
    /// ## Properties
    ///
    ///
    /// #### `state`
    ///  The current viewfinder state.
    /// The state indicates what the viewfinder is currently doing, or sometimes that an error has
    /// occurred. Many operations, such as taking a picture, require that the viewfinder be in the
    /// [`ViewfinderState::Ready`][crate::ViewfinderState::Ready] state.
    ///
    ///  Readable
    ///
    ///
    /// #### `detect-codes`
    ///  Whether the viewfinder should detect codes.
    /// When a code is detected, the [`code-detected`](#code-detected) signal will be emitted.
    ///
    ///  Readable | Writable
    ///
    ///
    /// #### `camera`
    ///  The camera that is currently being used.
    /// The [`DeviceProvider`][crate::DeviceProvider] handles obtaining new cameras,
    /// do not create cameras yourself.
    ///
    /// To safely switch cameras, the current [`fn@Viewfinder::state`] must be in [`ViewfinderState::Ready`][crate::ViewfinderState::Ready].
    /// This is because switching camera sources would interrupt most active operations, if any are present.
    ///
    ///  Readable | Nullable
    ///
    ///
    /// ## Signals
    ///
    ///
    /// #### `picture-done`
    ///  This signal is emitted after a picture has been taken and saved.
    /// Note that this signal is emitted even if saving the picture failed, and should not be used
    /// to detect if the picture was successfully saved.
    ///
    ///
    /// #### `recording-done`
    ///  This signal is emitted after a recording has finished and been saved.
    /// Note that this signal is emitted even if saving the recording failed, and should not be used
    /// to detect if the recoding was successfully saved.
    ///
    ///
    /// #### `code-detected`
    ///  This signal is emitted when a barcode is detected in the camera feed.
    /// This will only be emitted if [`detect-codes`](#detect-codes) is `true`.
    ///
    /// Barcodes are only detected when they appear on the feed, not on every frame when they are visible.
    ///
    /// # Implements
    ///
    /// [`gtk::prelude::WidgetExt`][trait@gtk::prelude::WidgetExt], [`glib::ObjectExt`][trait@gtk::glib::ObjectExt]
    pub struct Viewfinder(ObjectSubclass<imp::Viewfinder>)
        @extends gtk::Widget;
}

impl Default for Viewfinder {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Viewfinder {
    /// Creates a new [`Viewfinder`][crate::Viewfinder]
    ///
    /// # Returns
    ///
    /// a new Viewfinder
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the aspect ratio of the camera output.
    ///
    /// # Returns
    ///
    /// an aspect ratio calculated with width/height, or 0 for no valid aspect
    /// ratio.
    pub fn aspect_ratio(&self) -> f64 {
        let imp = self.imp();
        if let Some(paintable) = imp.picture.paintable() {
            paintable.intrinsic_aspect_ratio()
        } else {
            0.0
        }
    }

    /// Takes a picture.
    ///
    /// The recording will be saved to `location`. This method throws an error
    /// if:
    ///  - we are already recording or taking a picture
    ///  - the [`fn@Viewfinder::state`] of the camera is not
    ///    [`ViewfinderState::Ready`][crate::ViewfinderState::Ready].
    ///
    /// This operation may take a while. The resolution might be changed
    /// temporarily, autofocusing might take place, etc. Basically
    /// everything you'd expect to happen when you click the photo button in
    /// a camera app.
    ///
    /// The [`picture-done`](#picture-done) signal will be emitted when this
    /// operation ends.
    pub fn take_picture<P: AsRef<Path>>(&self, location: P) -> Result<(), crate::CaptureError> {
        let imp = self.imp();

        if !matches!(self.state(), ViewfinderState::Ready) {
            return Err(crate::CaptureError::NotReady);
        }

        if imp.is_taking_picture.get() {
            return Err(crate::CaptureError::SnapshotInProgress);
        }

        if imp.is_recording_video.borrow().is_some() {
            return Err(crate::CaptureError::RecordingInProgress);
        }

        // Set after we cannot fail anymore.
        imp.is_taking_picture.set(true);

        self.set_tags();

        let camerabin = imp.camerabin();
        camerabin.set_property_from_str("mode", "mode-image");
        camerabin.set_property("location", location.as_ref().display().to_string());
        camerabin.emit_by_name::<()>("start-capture", &[]);

        Ok(())
    }

    /// Starts recording a video.
    ///
    /// The recording will be saved to `location`. This method throws an error
    /// if:
    ///  - we are already recording or taking a picture
    ///  - the [`fn@Viewfinder::state`] of the camera is not
    ///    [`ViewfinderState::Ready`][crate::ViewfinderState::Ready].
    pub fn start_recording<P: AsRef<Path>>(&self, location: P) -> Result<(), crate::CaptureError> {
        let imp = self.imp();

        if !matches!(self.state(), ViewfinderState::Ready) {
            return Err(crate::CaptureError::NotReady);
        }

        if imp.is_taking_picture.get() {
            return Err(crate::CaptureError::SnapshotInProgress);
        }

        if imp.is_recording_video.borrow().is_some() {
            return Err(crate::CaptureError::RecordingInProgress);
        }

        // Set after we cannot fail anymore.
        if !imp
            .is_recording_video
            .replace(Some(location.as_ref().to_owned()))
            .is_some_and(|old| old == location.as_ref())
        {
            self.notify_is_recording();
        };

        let camerabin = imp.camerabin();
        camerabin.set_property_from_str("mode", "mode-video");
        camerabin.set_property("location", location.as_ref().display().to_string());

        self.set_tags();

        camerabin.emit_by_name::<()>("start-capture", &[]);

        Ok(())
    }

    /// Stop recording video.
    ///
    /// This method throws an error if:
    /// - [`fn@Viewfinder::start_recording`] hasn't been called
    /// - There is another [`fn@Viewfinder::stop_recording`] operation in
    ///   progress.
    ///
    /// The [`recording-done`](#recording-done) signal will be emitted when this
    /// operation ends.
    pub fn stop_recording(&self) -> Result<(), crate::CaptureError> {
        let imp = self.imp();

        if !imp.is_recording_video.borrow().is_some() {
            return Err(crate::CaptureError::NoRecordingToStop);
        }

        if imp.is_stopping_recording.get() {
            return Err(crate::CaptureError::StopRecordingInProgress);
        }

        imp.is_stopping_recording.set(true);

        imp.camerabin().emit_by_name::<()>("stop-capture", &[]);

        Ok(())
    }

    pub fn connect_picture_done<F: Fn(&Self, Option<&gio::File>) + 'static>(&self, f: F) {
        self.connect_closure(
            "picture-done",
            false,
            glib::closure_local!(|obj, file| {
                f(obj, file);
            }),
        );
    }

    pub fn connect_recording_done<F: Fn(&Self, Option<&gio::File>) + 'static>(&self, f: F) {
        self.connect_closure(
            "recording-done",
            false,
            glib::closure_local!(|obj, file| {
                f(obj, file);
            }),
        );
    }

    pub fn connect_code_detected<F: Fn(&Self, crate::CodeType, &str) + 'static>(&self, f: F) {
        self.connect_closure(
            "code-detected",
            false,
            glib::closure_local!(|obj, data_type, data| {
                f(obj, data_type, data);
            }),
        );
    }

    /// Starts the viewfinder.
    pub fn start_stream(&self) {
        glib::spawn_future_local(glib::clone!(@weak self as obj => async move {
            obj.change_state_inner(gst::State::Playing).await;
        }));
    }

    // It is not needed to call this for gst::State::Null.
    async fn change_state_inner(&self, state: gst::State) {
        let (sender, receiver) = futures_channel::oneshot::channel();

        let camerabin = self.imp().camerabin();
        std::thread::spawn(glib::clone!(@weak camerabin => move || {
            let timeout = gst::format::ClockTime::from_seconds(2);
            let (res, current_state, pending_state) = camerabin.state(Some(timeout));
            let new_state_is = match res {
                Ok(change_done) => {
                    if change_done == gst::StateChangeSuccess::Async {
                        camerabin.set_locked_state(true);
                        log::debug!("Camerabin could not change its state from {current_state:?} to {pending_state:?}");
                        StateChangeState::NotDone
                    } else if current_state == state {
                        StateChangeState::Equal
                    } else {
                        StateChangeState::Differ
                    }
                }
                Err(err) => {
                    log::error!("Previous camerabin state changed failed: {err}");
                    StateChangeState::Error
                }
            };
            sender.send(new_state_is).unwrap();
        }))
            .join()
            .unwrap();

        let change_state = receiver.await.unwrap();
        match change_state {
            StateChangeState::Equal => {
                // Nothing to do, the new state matches the current one.
            }
            StateChangeState::NotDone => {
                log::debug!("Aborting camerabin state change {state:?}");
                camerabin.abort_state();
                camerabin.set_locked_state(false);
                self.set_camerabin_state(state);
            }
            // If the previous state change failed, we might as well try to set it now.
            StateChangeState::Error => self.set_camerabin_state(state),
            StateChangeState::Differ => self.set_camerabin_state(state),
        }
    }

    fn set_camerabin_state(&self, state: gst::State) {
        match self.imp().camerabin().set_state(state) {
            Err(err) => {
                log::error!("Could not start camerabin: {err}");
                self.imp().set_state(ViewfinderState::Error);
            }
            Ok(gst::StateChangeSuccess::Async) => {
                log::debug!("Trying to set camerabin state to {state:?}")
            }
            Ok(_) => log::debug!("Camerabin succesfully state set to {state:?}"),
        }
    }

    /// Stops the viewfinder.
    ///
    /// A black frame will be shown after this methods has been called.
    pub fn stop_stream(&self) {
        if let Err(err) = self.imp().camerabin().set_state(gst::State::Null) {
            log::error!("Could not pause camerabin: {err}");
            self.imp().set_state(ViewfinderState::Error);
        } else {
            log::debug!("Camerabin state succesfully set to NULL");
        }
    }

    /// Bus message handler for the pipeline
    fn on_bus_message(&self, msg: &gst::Message) {
        match msg.view() {
            gst::MessageView::Error(msg) => self.on_pipeline_error(msg),
            gst::MessageView::Element(msg) => match msg.structure() {
                Some(s) if s.has_name("image-done") => {
                    let path = s.get::<PathBuf>("filename").unwrap();
                    let file = gio::File::for_path(path);
                    self.on_image_done(&file);
                }
                Some(s) if s.has_name("video-done") => {
                    self.on_video_done();
                }
                Some(s) if s.has_name("barcode") => {
                    let type_str = s.get::<&str>("type").unwrap();
                    let data_type = type_str.into();
                    let data = s.get::<&str>("symbol").unwrap();

                    self.on_barcode_detected(data_type, data);
                }
                _ => (),
            },
            _ => (),
        }
    }

    fn on_image_done(&self, file: &gio::File) {
        self.imp().is_taking_picture.set(false);

        self.emit_picture_done(Some(file));
    }

    fn on_video_done(&self) {
        self.imp().is_stopping_recording.set(false);

        if let Some(path) = self.imp().is_recording_video.take() {
            self.notify_is_recording();
            let file = gio::File::for_path(path);
            self.emit_recording_done(Some(&file));
        }
    }

    fn on_barcode_detected(&self, data_type: crate::CodeType, data: &str) {
        // We don't emit the signal if we just emited it
        if self.imp().timeout_handler.borrow().is_none() {
            let id = glib::timeout_add_seconds_local_once(
                BARCODE_TIMEOUT,
                glib::clone!(@weak self as obj => move || {
                    obj.imp().timeout_handler.take();
                }),
            );
            self.imp().timeout_handler.replace(Some(id));
            self.emit_code_detected(data_type, data);
        }
    }

    fn on_pipeline_error(&self, err: &gst::message::Error) {
        log::error!(
            "Bus Error from {:?}\n{}\n{:?}",
            err.src().map(|s| s.path_string()),
            err.error(),
            err.debug()
        );

        self.cancel_current_operation();

        if self.imp().camerabin().current_state() != gst::State::Playing {
            self.imp().set_state(ViewfinderState::Error);
        }
    }

    fn cancel_current_operation(&self) {
        let imp = self.imp();

        if imp.is_taking_picture.replace(false) {
            self.emit_picture_done(None);
        }
        if imp.is_recording_video.replace(None).is_some() {
            self.notify_is_recording();
            self.emit_recording_done(None);
        }
        imp.is_stopping_recording.set(false);
    }

    fn emit_picture_done(&self, file: Option<&gio::File>) {
        self.emit_by_name::<()>("picture-done", &[&file]);
    }

    fn emit_recording_done(&self, file: Option<&gio::File>) {
        self.emit_by_name::<()>("recording-done", &[&file]);
    }

    fn emit_code_detected(&self, data_type: crate::CodeType, data: &str) {
        self.emit_by_name::<()>("code-detected", &[&data_type, &data]);
    }

    fn set_tags(&self) {
        let imp = self.imp();

        let tagsetter = imp
            .camerabin()
            .dynamic_cast_ref::<gst::TagSetter>()
            .unwrap();
        tagsetter.add_tag::<gst::tags::ApplicationName>(
            crate::APP_ID.get().unwrap(),
            gst::TagMergeMode::Replace,
        );

        if let Some(datetime) = gst::DateTime::new_now_local_time() {
            tagsetter.add_tag::<gst::tags::DateTime>(&datetime, gst::TagMergeMode::Replace);
        }
        if let Some(camera) = self.camera() {
            let device_model = camera.display_name();
            tagsetter.add_tag::<gst::tags::DeviceModel>(
                &device_model.as_str(),
                gst::TagMergeMode::Replace,
            );
        }
    }

    fn setup_recording(&self) {
        use gst_pbutils::encoding_profile::EncodingProfileBuilder;

        let video_profile =
            gst_pbutils::EncodingVideoProfile::builder(&gst::Caps::builder("video/x-vp8").build())
                .preset("Profile Realtime")
                .variable_framerate(true)
                .build();
        let audio_profile = gst_pbutils::EncodingAudioProfile::builder(
            &gst::Caps::builder("audio/x-vorbis").build(),
        )
        .build();
        let profiles = gst_pbutils::EncodingContainerProfile::builder(
            &gst::Caps::builder("video/webm").build(),
        )
        .name("WebM audio/video")
        .description("Standard WebM/VP8/Vorbis")
        .add_profile(video_profile)
        .add_profile(audio_profile)
        .build();

        let camerabin = self.imp().camerabin();

        camerabin.set_property("video-profile", profiles);
    }

    fn init(&self) {
        let imp = self.imp();
        let devices = imp.devices.get().unwrap();

        if let Some(camera) = devices.default_camera().or_else(|| devices.camera(0)) {
            if matches!(
                self.state(),
                ViewfinderState::NoCameras | ViewfinderState::Loading | ViewfinderState::Error
            ) {
                imp.set_state(ViewfinderState::Ready);
                self.set_camera(Some(camera));
            }
        }

        glib::timeout_add_local_once(
            std::time::Duration::from_secs(PROVIDER_TIMEOUT),
            glib::clone!(@weak self as obj => move || {
                if matches!(obj.state(), ViewfinderState::Loading) {
                    obj.imp().set_state(ViewfinderState::NoCameras);
                }
            }),
        );
    }

    fn create_camera_element(
        &self,
        device_src: &gst::Element,
    ) -> Result<gst::Element, glib::BoolError> {
        use gst::prelude::*;

        let bin = gst::Bin::new();

        let capsfilter = gst::ElementFactory::make("capsfilter").build()?;
        let decodebin3 = gst::ElementFactory::make("decodebin3").build()?;

        let videoflip = gst::ElementFactory::make("videoflip")
            .property_from_str("video-direction", "auto")
            .build()?;

        bin.add_many([device_src, &capsfilter, &decodebin3, &videoflip])?;
        gst::Element::link_many([device_src, &capsfilter, &decodebin3])?;

        self.imp().capsfilter.set(capsfilter).unwrap();

        let (sender, receiver) = futures_channel::oneshot::channel::<bool>();
        let sender = std::sync::Arc::new(std::sync::Mutex::new(Some(sender)));
        decodebin3.connect_pad_added(glib::clone!(@weak videoflip => move |_, pad| {
            if pad.stream().is_some_and(|stream| matches!(stream.stream_type(), gst::StreamType::VIDEO)) {
                let has_succeeded = pad.link(&videoflip.static_pad("sink").unwrap())
                                       .inspect_err(|err| {
                                           log::error!("Failed to link decodebin3:video_%u pad with videoflip:sink pad: {err}");
                                       })
                                       .is_ok();
                let mut guard = sender.lock().unwrap();
                if let Some(sender) = guard.take() {
                    let _ = sender.send(has_succeeded);
                }
            }
        }));

        glib::spawn_future_local(glib::clone!(@weak self as viewfinder => async move {
            let has_succeeded = receiver.await.unwrap_or_default();
            if !has_succeeded {
                viewfinder.imp().set_state(ViewfinderState::Error);
            }
        }));

        let pad = videoflip.static_pad("src").unwrap();
        let ghost_pad = gst::GhostPad::with_target(&pad)?;
        ghost_pad.set_active(true)?;

        bin.add_pad(&ghost_pad)?;

        let wrappercamerabinsrc = gst::ElementFactory::make("wrappercamerabinsrc")
            .property("video-source", &bin)
            .build()
            .expect("Missing GStreamer Bad Plug-ins");

        Ok(wrappercamerabinsrc)
    }

    fn setup_camera_element(&self, camera: &crate::Camera) -> Result<(), glib::BoolError> {
        let imp = self.imp();

        if let Some(element) = imp.camera_element.get() {
            camera.reconfigure(element)?;
        } else {
            let element = camera.create_element()?;

            let wrapper = self.create_camera_element(&element)?;
            imp.camerabin().set_property("camera-source", &wrapper);

            imp.camera_element.set(element).unwrap();
        }

        if let Some(capsfilter) = imp.capsfilter.get() {
            let caps = camera.best_caps();
            capsfilter.set_property("caps", &caps);
        }

        let is_front_camera = !matches!(camera.location(), crate::CameraLocation::Back);
        imp.is_front_camera.set(is_front_camera);

        Ok(())
    }
}

fn create_zbar_bin() -> Result<gst::Element, glib::BoolError> {
    let bin = gst::Bin::new();

    let videoconvert = gst::ElementFactory::make("videoconvert").build()?;
    let zbar = gst::ElementFactory::make("zbar").build()?;
    let fakesink = gst::ElementFactory::make("fakesink").build()?;

    bin.add_many([&videoconvert, &zbar, &fakesink]).unwrap();
    gst::Element::link_many([&videoconvert, &zbar, &fakesink]).unwrap();

    let pad = videoconvert.static_pad("sink").unwrap();
    let ghost_pad = gst::GhostPad::with_target(&pad).unwrap();
    ghost_pad.set_active(true).unwrap();
    bin.add_pad(&ghost_pad).unwrap();

    Ok(bin.upcast())
}
