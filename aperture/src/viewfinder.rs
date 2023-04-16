// SPDX-License-Identifier: GPL-3.0-or-later
use gst::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene};

use once_cell::sync::Lazy;
use std::path::Path;
use std::path::PathBuf;

use crate::ViewfinderState;

mod imp {
    use super::*;

    use glib::Properties;
    use once_cell::unsync::OnceCell;
    use std::cell::Cell;
    use std::cell::RefCell;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::Viewfinder)]
    pub struct Viewfinder {
        #[property(get, set = Self::set_state, explicit_notify, builder(Default::default()))]
        state: Cell<ViewfinderState>,
        #[property(get = Self::detect_codes, set = Self::set_detect_codes, explicit_notify)]
        detect_codes: Cell<bool>,
        #[property(get, set = Self::set_camera, nullable, explicit_notify)]
        camera: RefCell<Option<crate::Camera>>,

        pub zbar_branch: RefCell<Option<gst::Element>>,
        pub devices: OnceCell<crate::DeviceProvider>,
        pub camera_src: RefCell<Option<gst::Element>>,
        pub camerabin: OnceCell<gst::Element>,
        pub sink_paintable: OnceCell<gst::Element>,
        pub tee: OnceCell<crate::PipelineTee>,

        // TODO Port to gio::Task,
        pub is_recording_video: RefCell<Option<PathBuf>>,
        pub is_stopping_recording: Cell<bool>,
        pub is_taking_picture: Cell<bool>,

        picture: gtk::Picture,
    }

    impl Viewfinder {
        pub fn camerabin(&self) -> &gst::Element {
            self.camerabin.get().unwrap()
        }

        fn set_state(&self, state: ViewfinderState) {
            if state != self.state.replace(state) {
                self.obj().notify_state();
            }
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

            if !matches!(obj.state(), ViewfinderState::Ready) {
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

            if obj.is_realized() {
                self.camerabin().set_state(gst::State::Null).unwrap();
            }

            if let Some(camera) = camera {
                // `camera_src` might be `None`, which means the element was
                // reconfigured and we should keep using it

                // TODO This is incredibly not idiomatic.
                let guard = self.camera_src.borrow();
                if let Some((bin, camera_src)) = camera.source_element(guard.as_ref()) {
                    drop(guard);
                    self.camerabin().set_property("camera-source", &bin);
                    self.camera_src.replace(Some(camera_src));
                }
            }

            if obj.is_realized() {
                self.camerabin().set_state(gst::State::Playing).unwrap();
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

    impl ObjectImpl for Viewfinder {
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

            crate::ensure_init();

            let obj = self.obj();

            let camerabin = gst::ElementFactory::make("camerabin").build().unwrap();
            self.camerabin.set(camerabin.clone()).unwrap();

            let bus = self.camerabin().bus().unwrap();
            bus.add_watch_local(
                glib::clone!(@weak obj => @default-return glib::Continue(false), move |_, msg| {
                    obj.on_bus_message(msg);

                    glib::Continue(true)
                }),
            )
            .unwrap();

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
                    .unwrap()
            } else {
                let bin = gst::Bin::default();
                let convert = gst::ElementFactory::make("videoconvert").build().unwrap();

                bin.add(&convert).unwrap();
                bin.add(&paintablesink).unwrap();
                convert.link(&paintablesink).unwrap();

                bin.add_pad(
                    &gst::GhostPad::with_target(Some("sink"), &convert.static_pad("sink").unwrap())
                        .unwrap(),
                )
                .unwrap();

                bin.upcast()
            };

            tee.add_branch(&sink);
            camerabin.set_property("viewfinder-sink", &tee);

            self.sink_paintable.set(paintablesink).unwrap();

            self.picture.set_hexpand(true);
            self.picture.set_vexpand(true);
            self.picture.set_content_fit(gtk::ContentFit::Fill);
            self.picture.set_parent(&*obj);
            self.picture.set_paintable(Some(&paintable));

            self.tee.set(tee).unwrap();

            let devices = crate::DeviceProvider::instance();
            if let Err(err) = devices.start() {
                log::error!("Could not start device provider: {err}");
                obj.set_state(ViewfinderState::Error);
            };

            devices.connect_camera_added(glib::clone!(@weak obj => move |_, camera| {
                if matches!(obj.state(), ViewfinderState::NoCameras) {
                    obj.set_state(ViewfinderState::Ready);
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
                        obj.set_state(ViewfinderState::NoCameras);
                    }
                }
            }));

            if let Some(camera) = devices.camera(0) {
                obj.set_state(ViewfinderState::Ready);
                obj.set_camera(Some(camera));
            } else {
                obj.set_state(ViewfinderState::NoCameras);
            }

            self.devices.set(devices.clone()).unwrap();

            obj.setup_recording();
        }

        fn dispose(&self) {
            if self.is_recording_video.borrow().is_some() {
                if let Err(err) = self.obj().stop_recording() {
                    log::error!("Could not stop recording: {err}");
                }
            }
            self.picture.unparent();
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![
                    // These are emited whenever the saving process finishes,
                    // successful or not.
                    glib::subclass::Signal::builder("picture-done")
                        .param_types([gio::File::static_type()])
                        .build(),
                    glib::subclass::Signal::builder("recording-done")
                        .param_types([gio::File::static_type()])
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

            self.camerabin().set_state(gst::State::Playing).unwrap();
        }

        fn unrealize(&self) {
            self.camerabin().set_state(gst::State::Null).unwrap();

            self.parent_unrealize();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let w = self.obj().width() as f32;

            // TODO Use is_some_and when stabilized.
            let should_mirror = self
                .obj()
                .camera()
                .map(|camera| matches!(camera.location(), crate::CameraLocation::Front))
                == Some(true);

            // This is the composition of translate (-w / 2.0, 0.0), map x to
            // -x, and translate (w / 2.0 , 0.0). Note that gsk matrices are
            // transposed (they act on row vectors).
            if should_mirror {
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
    pub struct Viewfinder(ObjectSubclass<imp::Viewfinder>)
        @extends gtk::Widget;
}

impl Default for Viewfinder {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Viewfinder {
    /// Creates a new `ApertureViewfinder`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes a picture.
    ///
    /// This may take a while. The resolution might be changed temporarily,
    /// autofocusing might take place, etc. Basically everything you'd expect
    /// to happen when you click the photo button in a camera app.
    ///
    /// The `image-done` signal will be emited with this operation ends.
    pub fn take_picture<P: AsRef<Path>>(&self, location: P) -> anyhow::Result<()> {
        let imp = self.imp();

        if !matches!(self.state(), ViewfinderState::Ready) {
            Err(crate::CaptureError::NotReady)?;
        }

        if imp.is_taking_picture.get() {
            Err(crate::CaptureError::SnapshotInProgress)?;
        }

        if imp.is_recording_video.borrow().is_some() {
            Err(crate::CaptureError::RecordingInProgress)?;
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
    /// The video will be saved to `location`. This method throws an error if:
    /// we are already recording, taking a picture, or the [`Self::state()`] of
    /// the camera is not [`crate::ViewfinderState::Ready`].
    ///
    /// The `video-done` signal will be emited with this operation ends.
    pub fn start_recording<P: AsRef<Path>>(&self, location: P) -> anyhow::Result<()> {
        let imp = self.imp();

        if !matches!(self.state(), ViewfinderState::Ready) {
            Err(crate::CaptureError::NotReady)?;
        }

        if imp.is_taking_picture.get() {
            Err(crate::CaptureError::SnapshotInProgress)?;
        }

        if imp.is_recording_video.borrow().is_some() {
            Err(crate::CaptureError::RecordingInProgress)?;
        }

        // Set after we cannot fail anymore.
        imp.is_recording_video
            .replace(Some(location.as_ref().to_owned()));

        let camerabin = imp.camerabin();
        camerabin.set_property_from_str("mode", "mode-video");
        camerabin.set_property("location", location.as_ref().display().to_string());

        self.set_tags();

        camerabin.emit_by_name::<()>("start-capture", &[]);

        Ok(())
    }

    /// Stop recording video.
    ///
    /// Will error out if [`Self::start_recording()`] hasn't been called or if
    /// there is another [`Self::stop_recording()`] call in progress.
    pub fn stop_recording(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        if !imp.is_recording_video.borrow().is_some() {
            Err(crate::CaptureError::NoRecordingToStop)?;
        }

        if imp.is_stopping_recording.get() {
            Err(crate::CaptureError::StopRecordingInProgress)?;
        }

        imp.is_stopping_recording.set(true);

        imp.camerabin().emit_by_name::<()>("stop-capture", &[]);

        Ok(())
    }

    /// Whether a recording is in progress.
    pub fn is_recording(&self) -> bool {
        self.imp().is_recording_video.borrow().is_some()
    }

    pub fn connect_picture_done<F: Fn(&Self, &gio::File) + 'static>(&self, f: F) {
        self.connect_closure(
            "picture-done",
            false,
            glib::closure_local!(|obj, file| {
                f(obj, file);
            }),
        );
    }

    pub fn connect_recording_done<F: Fn(&Self, &gio::File) + 'static>(&self, f: F) {
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

        self.emit_picture_done(file);
    }

    fn on_video_done(&self) {
        self.imp().is_stopping_recording.set(false);

        if let Some(path) = self.imp().is_recording_video.take() {
            let file = gio::File::for_path(path);
            self.emit_recording_done(&file);
        }
    }

    fn on_barcode_detected(&self, data_type: crate::CodeType, data: &str) {
        self.emit_code_detected(data_type, data);
    }

    fn on_pipeline_error(&self, err: &gst::message::Error) {
        log::error!(
            "Error from {:?}: {} ({:?})",
            err.src().map(|s| s.path_string()),
            err.error(),
            err.debug()
        );

        self.cancel_current_operation();

        if self.imp().camerabin().current_state() != gst::State::Playing {
            self.set_state(ViewfinderState::Error);
        }
    }

    fn cancel_current_operation(&self) {
        let imp = self.imp();

        imp.is_taking_picture.set(false);
        imp.is_recording_video.replace(None);
        imp.is_stopping_recording.set(false);
    }

    fn emit_picture_done(&self, file: &gio::File) {
        self.emit_by_name::<()>("picture-done", &[&file]);
    }

    fn emit_recording_done(&self, file: &gio::File) {
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

        // TODO Which parts here are necessary?
        // let preview_caps = gst::Caps::builder("video/x-raw")
        //     .field("format", "RGB")
        //     .build();

        // let mut capture_caps = gst::Caps::builder("video/webm")
        //     .field("framerate", gst::Fraction::new(60, 1))
        //     .field("width", 640)
        //     .field("height", 320)
        //     .build();

        // camerabin.set_property("viewfinder-caps", &capture_caps);
        // camerabin.set_property("image-capture-caps", &capture_caps);
        // capture_caps.fixate();
        // camerabin.set_property("video-capture-caps", &capture_caps);
        // camerabin.set_property("preview-caps", &preview_caps);

        camerabin.set_property("video-profile", profiles);
    }
}

fn create_zbar_bin() -> anyhow::Result<gst::Element> {
    let bin = gst::Bin::new(None);

    let videoconvert = gst::ElementFactory::make("videoconvert").build()?;
    let zbar = gst::ElementFactory::make("zbar").build()?;
    let fakesink = gst::ElementFactory::make("fakesink").build()?;

    bin.add_many(&[&videoconvert, &zbar, &fakesink]).unwrap();
    gst::Element::link_many(&[&videoconvert, &zbar, &fakesink]).unwrap();

    let pad = videoconvert.static_pad("sink").unwrap();
    let ghost_pad = gst::GhostPad::with_target(Some("sink"), &pad).unwrap();
    ghost_pad.set_active(true).unwrap();
    bin.add_pad(&ghost_pad).unwrap();

    Ok(bin.upcast())
}
