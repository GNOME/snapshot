// SPDX-License-Identifier: GPL-3.0-or-later
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;

use gst::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene};

use crate::VideoFormat;
use crate::ViewfinderState;
use crate::code_detector::QrCodeDetector;

/// Default bitrate
///
/// This is the Gstreamer 1.26 default value for x264enc, chosen as reasonable compromise between
/// quality and file size. Candidate for a preference.
const DEFAULT_BITRATE: u32 = 2048;
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
        #[property(get, explicit_notify, default)]
        state: Cell<ViewfinderState>,
        #[property(get = Self::detect_codes, set = Self::set_detect_codes, explicit_notify)]
        detect_codes: Cell<bool>,
        #[property(get, set = Self::set_camera, nullable, explicit_notify)]
        camera: RefCell<Option<crate::Camera>>,
        #[property(get = Self::is_recording, name = "is-recording", type = bool)]
        pub is_recording_video: RefCell<Option<PathBuf>>,
        #[property(get, set = Self::set_disable_audio_recording, explicit_notify)]
        disable_audio_recording: Cell<bool>,
        #[property(get, set = Self::set_video_format, explicit_notify, default)]
        video_format: Cell<VideoFormat>,
        #[property(get, set = Self::set_enable_hw_encoding, explicit_notify)]
        enable_hw_encoding: Cell<bool>,

        pub qrcode_branch: RefCell<Option<gst::Element>>,
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
        pub offload: gtk::GraphicsOffload,
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
            self.qrcode_branch.borrow().is_some()
        }

        fn set_detect_codes(&self, value: bool) {
            if value == self.detect_codes.replace(value) {
                return;
            }

            let tee = self.tee.get().unwrap();
            if value {
                match create_qrcode_bin() {
                    Ok(qrcode_branch) => {
                        tee.add_branch(&qrcode_branch);
                        self.qrcode_branch.replace(Some(qrcode_branch));
                    }
                    Err(err) => {
                        log::error!("Could not create qrcode element: {err}");
                    }
                }
            } else if let Some(qrcode_branch) = self.qrcode_branch.take() {
                tee.remove_branch(&qrcode_branch);
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

            if let Some(camera) = camera
                && let Err(err) = obj.setup_camera_element(&camera)
            {
                log::error!("Could not reconfigure camera element: {err}");
                self.set_state(ViewfinderState::Error);
            }

            if obj.is_realized() && matches!(obj.state(), ViewfinderState::Ready) {
                obj.start_stream();
            }

            obj.notify_camera();
        }

        fn set_disable_audio_recording(&self, value: bool) {
            let obj = self.obj();

            if value != self.disable_audio_recording.replace(value) {
                obj.reset_pipeline();
                obj.notify_disable_audio_recording();
            }
        }

        fn set_video_format(&self, video_format: VideoFormat) {
            let obj = self.obj();

            if video_format != self.video_format.replace(video_format) {
                obj.reset_pipeline();
                obj.notify_video_format();
            }
        }

        fn set_enable_hw_encoding(&self, value: bool) {
            let obj = self.obj();

            if value != self.enable_hw_encoding.replace(value) {
                match self.video_format.get() {
                    VideoFormat::Vp8Webm => (),
                    VideoFormat::H264Mp4 => obj.reset_pipeline(),
                }
                obj.notify_enable_hw_encoding();
            }
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
            let watch = bus
                .add_watch_local(glib::clone!(
                    #[weak]
                    obj,
                    #[upgrade_or]
                    glib::ControlFlow::Break,
                    move |_, msg| {
                        obj.on_bus_message(msg);
                        glib::ControlFlow::Continue
                    }
                ))
                .unwrap();
            self.bus_watch.set(watch).unwrap();

            let tee = crate::PipelineTee::new();

            let paintablesink = gst::ElementFactory::make("gtk4paintablesink")
                .build()
                .expect("Missing gst-plugin-gtk4");

            let paintable = paintablesink.property::<gdk::Paintable>("paintable");

            let is_yuv_natively_supported = {
                let yuv_caps =
                    gst_video::video_make_raw_caps(&[gst_video::VideoFormat::Yuy2]).build();
                !paintablesink
                    .pad_template("sink")
                    .unwrap()
                    .caps()
                    .intersect(&yuv_caps)
                    .is_empty()
            };
            let sink = if is_yuv_natively_supported {
                let bin = gst::Bin::default();

                bin.add(&paintablesink).unwrap();
                bin.add_pad(
                    &gst::GhostPad::with_target(&paintablesink.static_pad("sink").unwrap())
                        .unwrap(),
                )
                .unwrap();

                bin.upcast()
            } else {
                let is_gl_supported = paintable
                    .property::<Option<gdk::GLContext>>("gl-context")
                    .is_some();
                if is_gl_supported {
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
                }
            };

            tee.add_branch(&sink);
            camerabin.set_property("viewfinder-sink", &tee);

            let videoconvert_video = gst::ElementFactory::make("videoconvert")
                .build()
                .expect("Missing GStreamer Base Plug-ins");
            camerabin.set_property("video-filter", &videoconvert_video);

            let videoconvert_image = gst::ElementFactory::make("videoconvert")
                .build()
                .expect("Missing GStreamer Base Plug-ins");
            camerabin.set_property("image-filter", &videoconvert_image);

            self.sink_paintable.set(paintablesink).unwrap();

            self.picture
                .set_accessible_role(gtk::AccessibleRole::Presentation);
            self.picture.set_hexpand(true);
            self.picture.set_vexpand(true);
            self.picture.set_paintable(Some(&paintable));

            self.offload.set_child(Some(&self.picture));
            self.offload.set_parent(&*obj);
            self.offload.set_black_background(true);

            self.tee.set(tee).unwrap();

            let devices = crate::DeviceProvider::instance();

            self.devices.set(devices.clone()).unwrap();

            if devices.started() {
                obj.init();
            } else {
                devices.connect_started_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |_| {
                        obj.init();
                    }
                ));
            }

            devices.connect_camera_added(glib::clone!(
                #[weak]
                obj,
                move |_, camera| {
                    if matches!(
                        obj.state(),
                        ViewfinderState::NoCameras
                            | ViewfinderState::Loading
                            | ViewfinderState::Error
                    ) {
                        obj.imp().set_state(ViewfinderState::Ready);
                        obj.set_camera(Some(camera.clone()));
                    }
                }
            ));

            devices.connect_camera_removed(glib::clone!(
                #[weak]
                obj,
                move |devices, camera| {
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
                }
            ));

            obj.setup_recording();
        }

        fn dispose(&self) {
            if self.is_recording_video.borrow().is_some()
                && let Err(err) = self.obj().stop_recording()
            {
                log::error!("Could not stop recording: {err}");
            }
            if let Err(err) = self.camerabin().set_state(gst::State::Null) {
                log::error!("Could not stop camerabin: {err}");
            }

            self.offload.unparent();
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
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
                        .param_types([glib::Bytes::static_type()])
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
                log::debug!("Viewfinder realized: starting stream");
                self.obj().start_stream();
            }
        }

        fn unrealize(&self) {
            log::debug!("Viewfinder unrealized: stopping stream");
            self.obj().stop_stream();

            self.parent_unrealize();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if self.is_front_camera.get() {
                let w = self.obj().width() as f32;
                snapshot.save();
                snapshot.translate(&graphene::Point::new(w, 0.0));
                snapshot.scale(-1.0, 1.0);
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
    /// ### `video-format`
    /// The video format for recordings. `[crate::is_h264_encoding_supported]`
    /// can be used to detect whether there is h264 support.
    ///
    ///  Readable | Writable
    ///
    /// ### `enable-hw-encoding`
    /// Whether to enable hardware video encoding.
    /// `[crate::is_hardware_encoding_supported]` can be used to detect whether
    /// the system supports hardware encoding for a given format.
    ///
    ///  Readable | Writable
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
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
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
        if imp
            .is_recording_video
            .replace(Some(location.as_ref().to_owned()))
            .is_none_or(|old| old != location.as_ref())
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

    pub fn connect_code_detected<F: Fn(&Self, glib::Bytes) + 'static>(&self, f: F) {
        self.connect_closure(
            "code-detected",
            false,
            glib::closure_local!(|obj, data| {
                f(obj, data);
            }),
        );
    }

    /// Starts the viewfinder.
    pub fn start_stream(&self) {
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                obj.change_state_inner(gst::State::Playing).await;
            }
        ));
    }

    // It is not needed to call this for gst::State::Null.
    async fn change_state_inner(&self, state: gst::State) {
        let (sender, receiver) = futures_channel::oneshot::channel();

        let camerabin = self.imp().camerabin();
        std::thread::spawn(glib::clone!(#[weak] camerabin, move || {
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
            StateChangeState::Error | StateChangeState::Differ => self.set_camerabin_state(state),
        }
    }

    fn set_camerabin_state(&self, state: gst::State) {
        match self.imp().camerabin().set_state(state) {
            Err(err) => {
                log::error!("Could not start camerabin: {err}");
                self.imp().set_state(ViewfinderState::Error);
            }
            Ok(gst::StateChangeSuccess::Async) => {
                log::debug!("Trying to set camerabin state to {state:?}");
            }
            Ok(_) => log::debug!("Camerabin successfully state set to {state:?}"),
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
            log::debug!("Camerabin state successfully set to NULL");
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
                Some(s) if s.has_name("qrcode") => {
                    let data = s.get::<glib::Bytes>("payload").unwrap();

                    self.emit_code_detected(data);
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

    fn emit_code_detected(&self, data: glib::Bytes) {
        log::info!("Code detected: {}", String::from_utf8_lossy(&data));
        self.emit_by_name::<()>("code-detected", &[&data]);
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
        use gst_pbutils::{ElementProperties, ElementPropertiesMapItem};

        // Video encoder properties
        let video_properties_map = ElementProperties::builder_map()
            .item(
                ElementPropertiesMapItem::builder("x264enc")
                    .field("bitrate", DEFAULT_BITRATE)
                    // tune "zerolatency": Suitable for live-sources like cameras. Crucial to avoid
                    //                     draining the buffer pool.
                    .field("tune", 4)
                    // speed-preset "faster": Lower CPU usage compared to the default "medium" with
                    //                        minimal reduction of quality, see
                    //                        https://streaminglearningcenter.com/wp-content/uploads/2019/10/Choosing-an-x264-Preset_1.pdf
                    .field("speed-preset", 4)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("openh264enc")
                    .field("bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vah264lpenc")
                    .field("bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vah264enc")
                    .field("bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vulkanh264enc")
                    .field("bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vp8enc")
                    .field("target-bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vavp8lpenc")
                    .field("bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vavp8enc")
                    .field("bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .build();

        let image_properties_map = ElementProperties::builder_map()
            .item(
                ElementPropertiesMapItem::builder("jpegenc")
                    .field("quality", 95)
                    // idct-method "float": Slowest, most accurate method.
                    .field("idct-method", 2)
                    .build(),
            )
            .build();

        let video_profile = match self.video_format() {
            VideoFormat::H264Mp4 => {
                let mut hw_encoder_found = false;
                let registry = gst::Registry::get();
                if let Some(encoder) = registry.lookup_feature("vah264lpenc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 2);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                if let Some(encoder) = registry.lookup_feature("vah264enc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 1);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                if let Some(encoder) = registry.lookup_feature("v4l2h264enc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 1);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                log::debug!(
                    "Setting up recording with h264/mp4 profile {} hw acceleration",
                    if self.enable_hw_encoding() && hw_encoder_found {
                        "with"
                    } else {
                        "without"
                    }
                );

                let caps = gst::Caps::builder("video/quicktime").build();
                let mut container_profile = gst_pbutils::EncodingContainerProfile::builder(&caps)
                    .name("MP4 audio/video")
                    .description("Standard MP4/H264/MP3");

                let video_profile = gst_pbutils::EncodingVideoProfile::builder(
                    &gst::Caps::builder("video/x-h264").build(),
                )
                .variable_framerate(true)
                .element_properties(video_properties_map)
                .build();
                container_profile = container_profile.add_profile(video_profile);

                if !self.disable_audio_recording() {
                    let audio_profile = gst_pbutils::EncodingAudioProfile::builder(
                        &gst::Caps::builder("audio/mpeg").build(),
                    )
                    .build();
                    container_profile = container_profile.add_profile(audio_profile);
                }

                container_profile.build()
            }
            VideoFormat::Vp8Webm => {
                let mut hw_encoder_found = false;
                let registry = gst::Registry::get();
                if let Some(encoder) = registry.lookup_feature("vavp8lpenc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 2);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                if let Some(encoder) = registry.lookup_feature("vavp8enc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 1);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                if let Some(encoder) = registry.lookup_feature("v4l2vp8enc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 1);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                log::debug!(
                    "Setting up recording with vp8/webm profile {} hw acceleration",
                    if self.enable_hw_encoding() && hw_encoder_found {
                        "with"
                    } else {
                        "without"
                    }
                );

                let caps = gst::Caps::builder("video/webm").build();
                let mut container_profile = gst_pbutils::EncodingContainerProfile::builder(&caps)
                    .name("WebM audio/video")
                    .description("Standard WebM/VP8/Vorbis");

                let video_profile = gst_pbutils::EncodingVideoProfile::builder(
                    &gst::Caps::builder("video/x-vp8").build(),
                )
                .preset("Profile Realtime")
                .variable_framerate(true)
                .element_properties(video_properties_map)
                .build();
                container_profile = container_profile.add_profile(video_profile);

                if !self.disable_audio_recording() {
                    let audio_profile = gst_pbutils::EncodingAudioProfile::builder(
                        &gst::Caps::builder("audio/x-vorbis").build(),
                    )
                    .build();
                    container_profile = container_profile.add_profile(audio_profile);
                }

                container_profile.build()
            }
        };

        let image_profile =
            gst_pbutils::EncodingVideoProfile::builder(&gst::Caps::builder("image/jpeg").build())
                .variable_framerate(true)
                .element_properties(image_properties_map)
                .build();

        let camerabin = self.imp().camerabin();
        camerabin.set_property("video-profile", video_profile);
        camerabin.set_property("image-profile", image_profile);
    }

    fn init(&self) {
        let imp = self.imp();
        let devices = imp.devices.get().unwrap();

        if let Some(camera) = devices.default_camera().or_else(|| devices.camera(0))
            && matches!(
                self.state(),
                ViewfinderState::NoCameras | ViewfinderState::Loading | ViewfinderState::Error
            )
        {
            imp.set_state(ViewfinderState::Ready);
            self.set_camera(Some(camera));
        }

        glib::timeout_add_local_once(
            std::time::Duration::from_secs(PROVIDER_TIMEOUT),
            glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move || {
                    if matches!(obj.state(), ViewfinderState::Loading) {
                        obj.imp().set_state(ViewfinderState::NoCameras);
                    }
                }
            ),
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
        let capsfilter_post_decode = gst::ElementFactory::make("capsfilter").build()?;
        let caps_post_decode = gst::Caps::builder("video/x-raw").build();
        capsfilter_post_decode.set_property("caps", &caps_post_decode);

        bin.add_many([
            device_src,
            &capsfilter,
            &decodebin3,
            &capsfilter_post_decode,
        ])?;
        gst::Element::link_many([device_src, &capsfilter, &decodebin3])?;

        self.imp().capsfilter.set(capsfilter).unwrap();

        let (sender, receiver) = futures_channel::oneshot::channel::<bool>();
        let sender = std::sync::Arc::new(std::sync::Mutex::new(Some(sender)));
        decodebin3.connect_pad_added(glib::clone!(#[weak] capsfilter_post_decode, move |_, pad| {
            if pad.stream().is_some_and(|stream| matches!(stream.stream_type(), gst::StreamType::VIDEO)) {
                let has_succeeded = pad.link(&capsfilter_post_decode.static_pad("sink").unwrap())
                                       .inspect_err(|err| {
                                           log::error!("Failed to link decodebin3:video_%u pad with capsfilter_post_decode:sink pad: {err}");
                                       })
                                       .is_ok();
                let mut guard = sender.lock().unwrap();
                if let Some(sender) = guard.take() {
                    let _ = sender.send(has_succeeded);
                }
            }
        }));

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = viewfinder)]
            self,
            async move {
                let has_succeeded = receiver.await.unwrap_or_default();
                if !has_succeeded {
                    viewfinder.imp().set_state(ViewfinderState::Error);
                }
            }
        ));

        let pad = capsfilter_post_decode.static_pad("src").unwrap();
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

    fn reset_pipeline(&self) {
        if matches!(
            self.imp().camerabin().current_state(),
            gst::State::Playing | gst::State::Paused
        ) {
            self.stop_stream();
            self.setup_recording();
            self.start_stream();
        } else {
            self.setup_recording();
        }
    }
}

fn create_qrcode_bin() -> Result<gst::Element, glib::BoolError> {
    let bin = gst::Bin::new();

    let videorate = gst::ElementFactory::make("videorate").build()?;
    videorate.set_property("max-rate", 5);
    videorate.set_property("drop-only", true);
    let videoconvert = gst::ElementFactory::make("videoconvert").build()?;

    // Ensure a copy is made
    let capsfilter = gst::ElementFactory::make("capsfilter").build()?;
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", gst_video::VideoFormat::Gray8.to_str())
        .build();
    capsfilter.set_property("caps", &caps);

    let queue = gst::ElementFactory::make("queue").build()?;
    let qrcode = QrCodeDetector::new().upcast::<gst::Element>();
    let fakesink = gst::ElementFactory::make("fakesink").build()?;

    bin.add_many([
        &videorate,
        &videoconvert,
        &capsfilter,
        &queue,
        &qrcode,
        &fakesink,
    ])
    .unwrap();
    gst::Element::link_many([
        &videorate,
        &videoconvert,
        &capsfilter,
        &queue,
        &qrcode,
        &fakesink,
    ])
    .unwrap();

    let pad = videorate.static_pad("sink").unwrap();
    let ghost_pad = gst::GhostPad::with_target(&pad).unwrap();
    ghost_pad.set_active(true).unwrap();
    bin.add_pad(&ghost_pad).unwrap();

    Ok(bin.upcast())
}
