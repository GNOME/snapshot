// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use glib::clone;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::{config, utils};

mod imp {
    use std::cell::{Cell, RefCell};

    use glib::Properties;
    use once_cell::sync::{Lazy, OnceCell};

    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::CameraPaintable)]
    pub struct CameraPaintable {
        pub viewfinder: OnceCell<aperture::Viewfinder>,
        pub flash_bin: OnceCell<crate::FlashBin>,
        pub players: RefCell<Option<gtk::MediaFile>>,

        #[property(get, set = Self::set_transform, explicit_notify, builder(Default::default()))]
        transform: Cell<crate::Transform>,
    }

    impl CameraPaintable {
        fn set_transform(&self, transform: crate::Transform) {
            if transform != self.transform.replace(transform) {
                // TODO
                // self.pipeline.get().unwrap().set_transform(transform);
                self.obj().notify_transform();
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CameraPaintable {
        const NAME: &'static str = "CameraPaintable";
        type Type = super::CameraPaintable;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for CameraPaintable {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let viewfinder = aperture::Viewfinder::new();

            viewfinder.connect_picture_done(clone!(@weak obj => move |_, file| {
                obj.emit_picture_stored(Some(file));
            }));

            viewfinder.connect_recording_done(clone!(@weak obj => move |_, file| {
                obj.emit_video_stored(Some(file));
            }));

            let flash_bin = crate::FlashBin::default();
            flash_bin.set_parent(&*obj);

            flash_bin.set_child(Some(&viewfinder));
            self.flash_bin.set(flash_bin).unwrap();

            self.viewfinder.set(viewfinder).unwrap();
        }

        fn dispose(&self) {
            self.flash_bin.get().unwrap().unparent();
        }

        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            Self::derived_set_property(self, id, value, pspec)
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![
                    // These are emited whenever the saving process finishes,
                    // successful or not.
                    glib::subclass::Signal::builder("picture-stored")
                        .param_types([Option::<gio::File>::static_type()])
                        .build(),
                    glib::subclass::Signal::builder("video-stored")
                        .param_types([Option::<gio::File>::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for CameraPaintable {}
}

glib::wrapper! {
    pub struct CameraPaintable(ObjectSubclass<imp::CameraPaintable>) @extends gtk::Widget;
}

impl Default for CameraPaintable {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl CameraPaintable {
    pub fn set_camera(&self, camera: aperture::Camera) {
        let imp = self.imp();

        let viewfinder = imp.viewfinder.get().unwrap();
        // TODO unwrap
        viewfinder.set_camera(Some(camera)).unwrap();
    }

    pub fn take_snapshot(&self, format: crate::PictureFormat) -> anyhow::Result<()> {
        let imp = self.imp();

        let filename = utils::picture_file_name(format);
        let path = utils::pictures_dir()?.join(&filename);

        imp.viewfinder.get().unwrap().take_picture(path)?;
        imp.flash_bin.get().unwrap().flash();

        let settings = gio::Settings::new(config::APP_ID);
        if settings.boolean("play-shutter-sound") {
            self.play_shutter_sound();
        }

        Ok(())
    }

    // Start recording to the configured location
    pub fn start_recording(&self, format: crate::VideoFormat) -> anyhow::Result<()> {
        let filename = utils::video_file_name(format);
        let path = utils::videos_dir()?.join(filename);

        self.imp().viewfinder.get().unwrap().start_recording(path)?;

        Ok(())
    }

    // Stop recording if any recording was currently ongoing
    pub fn stop_recording(&self) {
        let viewfinder = self.imp().viewfinder.get().unwrap();
        if viewfinder.is_recording() {
            // TODO unwrap()
            viewfinder.stop_recording().unwrap();
        }
    }

    fn emit_picture_stored(&self, file: Option<&gio::File>) {
        self.emit_by_name::<()>("picture-stored", &[&file]);
    }

    fn emit_video_stored(&self, file: Option<&gio::File>) {
        self.emit_by_name::<()>("video-stored", &[&file]);
    }

    pub fn connect_picture_stored<F: Fn(&Self, Option<&gio::File>) + 'static>(&self, f: F) {
        self.connect_closure(
            "picture-stored",
            false,
            glib::closure_local!(|obj, file| {
                f(obj, file);
            }),
        );
    }

    pub fn connect_video_stored<F: Fn(&Self, Option<&gio::File>) + 'static>(&self, f: F) {
        self.connect_closure(
            "video-stored",
            false,
            glib::closure_local!(|obj, file| {
                f(obj, file);
            }),
        );
    }

    fn play_shutter_sound(&self) {
        // If we don't hold a reference to it there is a condition race which
        // will cause the sound to play only sometimes.
        let resource = "/org/gnome/Snapshot/sounds/camera-shutter.wav";
        let player = gtk::MediaFile::for_resource(resource);
        player.play();

        self.imp().players.replace(Some(player));
    }

    pub fn is_ready(&self) -> bool {
        let viewfinder = self.imp().viewfinder.get().unwrap();
        matches!(viewfinder.state(), aperture::ViewfinderState::Ready)
    }

    pub fn current_camera(&self) -> Option<aperture::Camera> {
        self.imp().viewfinder.get().unwrap().camera()
    }
}
