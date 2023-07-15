// SPDX-License-Identifier: GPL-3.0-or-later
use adw::subclass::prelude::*;
use adw::traits::BreakpointBinExt;
use ashpd::desktop::camera;
use gettextrs::gettext;
use gtk::{gio, glib};
use gtk::{prelude::*, CompositeTemplate};
use std::os::unix::io::RawFd;

use crate::{config, utils};

use super::CameraControls;

mod imp {
    use super::*;

    use once_cell::unsync::OnceCell;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Snapshot/ui/camera.ui")]
    pub struct Camera {
        pub stream_list: RefCell<gio::ListStore>,
        pub selection: gtk::SingleSelection,
        pub provider: OnceCell<aperture::DeviceProvider>,
        pub players: RefCell<Option<gtk::MediaFile>>,
        settings: OnceCell<gio::Settings>,

        pub recording_duration: Cell<u32>,
        pub recording_source: RefCell<Option<glib::source::SourceId>>,

        #[template_child]
        pub single_landscape_bp: TemplateChild<adw::Breakpoint>,
        #[template_child]
        pub dual_landscape_bp: TemplateChild<adw::Breakpoint>,
        #[template_child]
        pub dual_portrait_bp: TemplateChild<adw::Breakpoint>,

        #[template_child]
        pub recording_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub recording_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub viewfinder: TemplateChild<aperture::Viewfinder>,
        #[template_child]
        pub flash_bin: TemplateChild<crate::FlashBin>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub spinner: TemplateChild<gtk::Spinner>,

        #[template_child]
        pub guidelines: TemplateChild<crate::GuidelinesBin>,

        #[template_child]
        pub camera_controls_vertical: TemplateChild<crate::CameraControls>,
        #[template_child]
        pub camera_controls_horizontal: TemplateChild<crate::CameraControls>,

        #[template_child]
        pub sidebar_horizontal_end: TemplateChild<gtk::CenterBox>,
        #[template_child]
        pub sidebar_vertical_end: TemplateChild<gtk::CenterBox>,

        #[template_child]
        pub vertical_end_window_controls: TemplateChild<gtk::WindowControls>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Camera {
        const NAME: &'static str = "Camera";
        type Type = super::Camera;
        type ParentType = adw::BreakpointBin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
            klass.set_css_name("camera");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl Camera {
        pub fn settings(&self) -> &gio::Settings {
            self.settings
                .get_or_init(|| gio::Settings::new(config::APP_ID))
        }

        #[template_callback]
        fn change_breakpoint(&self, breakpoint: adw::Breakpoint) {
            let obj = self.obj();

            if breakpoint.eq(&self.dual_landscape_bp.get())
                || breakpoint.eq(&self.dual_portrait_bp.get())
            {
                obj.add_css_class("mobile");
            } else {
                obj.remove_css_class("mobile");
            }
        }
    }

    impl ObjectImpl for Camera {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let provider = aperture::DeviceProvider::instance();
            self.provider.set(provider.clone()).unwrap();

            provider.connect_camera_added(glib::clone!(@weak obj => move |provider, _| {
                obj.update_cameras_button(provider);
            }));
            provider.connect_camera_removed(glib::clone!(@weak obj => move |provider, _| {
                obj.update_cameras_button(provider);
            }));
            obj.update_cameras_button(provider);

            self.viewfinder
                .connect_state_notify(glib::clone!(@weak obj => move |_| {
                    obj.update_state();
                }));
            obj.update_state();

            self.selection.set_model(Some(provider));
            self.selection.connect_selected_item_notify(
                glib::clone!(@weak obj => move |selection| {
                    if let Some(selected_item) = selection.selected_item() {
                        let camera = selected_item.downcast::<aperture::Camera>().ok();

                        if matches!(obj.imp().viewfinder.state(), aperture::ViewfinderState::Ready | aperture::ViewfinderState::Error) {
                            obj.imp().viewfinder.set_camera(camera);
                        }
                    }
                }),
            );

            self.camera_controls_horizontal
                .set_selection(self.selection.clone());
            self.camera_controls_vertical
                .set_selection(self.selection.clone());

            self.camera_controls_horizontal.connect_camera_switched(
                glib::clone!(@weak self as obj => move |_: &CameraControls| {
                    obj.obj().camera_switched();
                }),
            );
            self.camera_controls_vertical.connect_camera_switched(
                glib::clone!(@weak self as obj => move |_: &CameraControls| {
                    obj.obj().camera_switched();
                }),
            );

            self.settings()
                .bind(
                    "show-composition-guidelines",
                    &*self.guidelines,
                    "draw-guidelines",
                )
                .build();

            // TODO remove if
            // https://gitlab.gnome.org/GNOME/gtk/-/merge_requests/5960 ever
            // lands.
            obj.update_window_controls();
            obj.settings().connect_gtk_decoration_layout_notify(
                glib::clone!(@weak obj => move |_| {
                    obj.update_window_controls();
                }),
            );

            obj.connect_current_breakpoint_notify(glib::clone!(@weak self as obj => move |imp| {
                if imp.current_breakpoint().is_none()
                || imp
                    .current_breakpoint()
                    .is_some_and(|breakpoint| breakpoint.eq(&obj.dual_portrait_bp.get()))
            {
                imp.add_css_class("portrait");
            } else {
                imp.remove_css_class("portrait");
            }
            }));
        }
    }

    impl WidgetImpl for Camera {}
    impl BreakpointBinImpl for Camera {}
}

glib::wrapper! {
    pub struct Camera(ObjectSubclass<imp::Camera>)
        @extends gtk::Widget, adw::BreakpointBin;
}

impl Default for Camera {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Camera {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn start(&self) {
        let provider = self.imp().provider.get().unwrap();

        let ctx = glib::MainContext::default();
        ctx.spawn_local(
            glib::clone!(@weak self as obj, @strong provider => async move {
                match stream().await {
                    Ok(fd) => {
                        if let Err(err) = provider.set_fd(fd) {
                            log::error!("Could not use the camera portal: {err}");
                        };
                    }
                    Err(err) => log::warn!("Could not use the camera portal: {err}"),
                }
                if let Err(err) = provider.start() {
                    log::error!("Could not start the device provider: {err}");
                } else {
                    log::debug!("Device provider started");
                };
            }),
        );
    }

    pub async fn start_recording(&self, format: crate::VideoFormat) -> anyhow::Result<()> {
        let filename = utils::video_file_name(format);
        let path = utils::videos_dir()?.join(filename);

        self.imp().viewfinder.start_recording(path)?;
        self.show_recording_label();

        Ok(())
    }

    pub fn stop_recording(&self) {
        let imp = self.imp();
        if matches!(imp.viewfinder.state(), aperture::ViewfinderState::Ready)
            && imp.viewfinder.is_recording()
        {
            if let Err(err) = imp.viewfinder.stop_recording() {
                log::error!("Could not stop camera: {err}");
            }
            self.hide_recording_label();
        }
    }

    pub async fn take_picture(&self, format: crate::PictureFormat) -> anyhow::Result<()> {
        let imp = self.imp();
        let window = self.root().and_downcast::<crate::Window>().unwrap();

        // We enable the shutter whenever picture-stored is emited.
        window.set_shutter_enabled(false);

        let filename = utils::picture_file_name(format);
        let path = utils::pictures_dir()?.join(filename);

        imp.viewfinder.take_picture(path)?;
        imp.flash_bin.flash();

        let settings = imp.settings();
        if settings.boolean("play-shutter-sound") {
            self.play_shutter_sound();
        }

        Ok(())
    }

    fn camera_switched(&self) {
        let provider = self.imp().provider.get().unwrap();

        let current = self.imp().viewfinder.camera();

        let mut pos = 0;
        if current == provider.camera(0) {
            pos += 1;
        };
        if let Some(camera) = provider.camera(pos) {
            self.imp().viewfinder.set_camera(Some(camera));
        }
    }

    fn play_shutter_sound(&self) {
        // If we don't hold a reference to it there is a condition race which
        // will cause the sound to play only sometimes.
        let resource = "/org/gnome/Snapshot/sounds/camera-shutter.wav";
        let player = gtk::MediaFile::for_resource(resource);
        player.play();

        self.imp().players.replace(Some(player));
    }

    fn active_controls(&self) -> &CameraControls {
        if self
            .current_breakpoint()
            .is_some_and(|bp| bp.to_string().contains("max-aspect-ratio: 4/3"))
        {
            &self.imp().camera_controls_horizontal
        } else {
            &self.imp().camera_controls_vertical
        }
    }

    pub fn set_countdown(&self, countdown: u32) {
        self.imp()
            .camera_controls_horizontal
            .set_countdown(countdown);
        self.imp().camera_controls_vertical.set_countdown(countdown);
    }

    pub fn start_countdown(&self) {
        self.imp().camera_controls_horizontal.start_countdown();
        self.imp().camera_controls_vertical.start_countdown();
    }

    pub fn stop_countdown(&self) {
        self.imp().camera_controls_horizontal.stop_countdown();
        self.imp().camera_controls_vertical.stop_countdown();
    }

    pub fn shutter_mode(&self) -> crate::ShutterMode {
        self.active_controls().shutter_mode()
    }

    pub fn set_shutter_mode(&self, shutter_mode: crate::ShutterMode) {
        if matches!(shutter_mode, crate::ShutterMode::Picture) {
            self.stop_recording();
        }
        self.imp()
            .camera_controls_horizontal
            .set_shutter_mode(shutter_mode);
        self.imp()
            .camera_controls_vertical
            .set_shutter_mode(shutter_mode);
    }

    pub fn set_gallery(&self, gallery: crate::Gallery) {
        let imp = self.imp();

        imp.viewfinder.connect_picture_done(
            glib::clone!(@weak gallery, @weak self as obj => move |_, file| {
                let window = obj.root().and_downcast::<crate::Window>().unwrap();
                window.set_shutter_enabled(true);
                // TODO Maybe report error via toast on None
                if let Some(file) = file {
                    gallery.add_image(file);
                }
            }),
        );
        imp.viewfinder.connect_recording_done(
            glib::clone!(@weak gallery, @weak self as obj => move |_, file| {
                let imp = obj.imp();
                // TODO Maybe report error via toast on None
                if let Some(file) = file {
                    gallery.add_video(file);
                }
                if matches!(imp.camera_controls_horizontal.shutter_mode(), crate::ShutterMode::Recording) {
                    imp.camera_controls_horizontal.set_shutter_mode(crate::ShutterMode::Video);
                    imp.camera_controls_vertical.set_shutter_mode(crate::ShutterMode::Video);
                }
            }),
        );
        imp.camera_controls_horizontal.set_gallery(&gallery);
        imp.camera_controls_vertical.set_gallery(&gallery);
    }

    pub fn toggle_guidelines(&self) {
        let imp = self.imp();

        imp.guidelines
            .set_draw_guidelines(!imp.guidelines.draw_guidelines());
    }

    fn update_cameras_button(&self, provider: &aperture::DeviceProvider) {
        self.imp()
            .camera_controls_horizontal
            .update_visible_camera_button(provider.n_items());
        self.imp()
            .camera_controls_vertical
            .update_visible_camera_button(provider.n_items());
    }

    fn update_state(&self) {
        let imp = self.imp();
        match imp.viewfinder.state() {
            aperture::ViewfinderState::Loading => {
                imp.spinner.start();
                imp.stack.set_visible_child_name("loading");
            }
            aperture::ViewfinderState::Ready => {
                imp.spinner.stop();
                imp.stack.set_visible_child_name("camera");
            }
            aperture::ViewfinderState::NoCameras => {
                imp.spinner.stop();
                imp.stack.set_visible_child_name("not-found")
            }
            aperture::ViewfinderState::Error => {
                imp.spinner.stop();
                imp.stack.set_visible_child_name("camera");

                let window = self.root().and_downcast::<crate::Window>().unwrap();
                window.send_toast(&gettext("Could not play camera stream"));
            }
        }
    }

    fn update_window_controls(&self) {
        let imp = self.imp();

        let decoration_layout = self.settings().gtk_decoration_layout().and_then(|layout| {
            layout
                .split_once(':')
                .map(|(_start, end)| end.split(',').rev().collect::<Vec<_>>().join(","))
        });
        imp.vertical_end_window_controls
            .set_decoration_layout(decoration_layout.as_deref());
    }

    fn show_recording_label(&self) {
        let imp = self.imp();

        let source = glib::timeout_add_seconds_local(
            1,
            glib::clone!(@weak self as obj =>  @default-return glib::Continue(false), move || {
                let imp = obj.imp();

                // TODO Use Cell::update once stabilized.
                let duration = imp.recording_duration.get() + 1;
                imp.recording_duration.set(duration);

                let minutes = duration.div_euclid(60);
                let seconds = duration.rem_euclid(60);
                imp.recording_label.set_label(&format!("{minutes}∶{seconds:02}"));

                glib::Continue(true)
            }),
        );
        if let Some(old) = imp.recording_source.replace(Some(source)) {
            old.remove();
        }
        imp.recording_duration.set(0);
        imp.recording_revealer.set_reveal_child(true);
        imp.recording_label.set_label("0∶00");
    }

    fn hide_recording_label(&self) {
        let imp = self.imp();

        if let Some(source) = imp.recording_source.take() {
            source.remove();
            imp.recording_duration.set(0);
            imp.recording_label.set_label("0∶00");
            imp.recording_revealer.set_reveal_child(false);
        }
    }
}

async fn stream() -> ashpd::Result<RawFd> {
    let proxy = camera::Camera::new().await?;
    proxy.request_access().await?;

    proxy.open_pipe_wire_remote().await
}
