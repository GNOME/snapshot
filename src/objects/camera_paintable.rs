// SPDX-License-Identifier: GPL-3.0-or-later
//
// Fancy Camera with QR code detection using ZBar
//
// Pipeline:
//                            queue -- videoconvert -- zbar -- fakesink
//                         /
//     pipewiresrc -- tee  -- queue2 -- gtkpaintablesink
//                         \
//                            queue3 -- fakesink2
use adw::prelude::*;
use glib::clone;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene};

use crate::config;
use crate::objects::Action;

/// Time to wait before trying to emit code-detected.
const CODE_TIMEOUT: u64 = 3;

mod imp {
    use std::cell::RefCell;

    use once_cell::sync::{Lazy, OnceCell};

    use super::*;

    #[derive(Debug, Default)]
    pub struct CameraPaintable {
        pub pipeline: OnceCell<crate::Pipeline>,
        pub sink_paintable: OnceCell<gdk::Paintable>,
        pub code: RefCell<Option<String>>,

        pub flash_ani: OnceCell<adw::TimedAnimation>,
        pub players: RefCell<Option<gtk::MediaFile>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CameraPaintable {
        const NAME: &'static str = "CameraPaintable";
        type Type = super::CameraPaintable;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for CameraPaintable {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let (sender, receiver) = glib::MainContext::channel::<Action>(glib::PRIORITY_DEFAULT);
            receiver.attach(
                None,
                clone!(@weak obj => @default-return glib::Continue(false), move |action| {
                    match action {
                        Action::CodeDetected(hash) => {
                            // FIXME This is a bad clone
                            if Some(&hash) != obj.imp().code.replace(Some(hash.clone())).as_ref() {
                                obj.emit_code_detected(&hash);

                                let duration = std::time::Duration::from_secs(CODE_TIMEOUT);
                                glib::timeout_add_local_once(duration, glib::clone!(@weak obj => move || {
                                    obj.imp().code.take();
                                }));
                            }
                        },
                        Action::PictureSaved(path) => {
                            let file = path.map(|path| gio::File::for_path(path));
                            obj.emit_picture_stored(file.as_ref());
                        },
                        Action::VideoSaved(path) => {
                            let file = path.map(|path| gio::File::for_path(path));
                            obj.emit_video_stored(file.as_ref());
                        },
                    }
                    return glib::Continue(true);
                }),
            );

            let pipeline = crate::Pipeline::new(sender);
            let paintable = pipeline.paintable();

            paintable.connect_invalidate_contents(clone!(@weak obj => move |_| {
                obj.invalidate_contents();
            }));

            paintable.connect_invalidate_size(clone!(@weak obj => move |_| {
                obj.invalidate_size();
            }));

            self.pipeline.set(pipeline).unwrap();
            self.sink_paintable.set(paintable).unwrap();
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![
                    glib::subclass::Signal::builder("code-detected")
                        .param_types([String::static_type()])
                        .build(),
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

    impl PaintableImpl for CameraPaintable {
        fn intrinsic_height(&self) -> i32 {
            if let Some(paintable) = self.sink_paintable.get() {
                paintable.intrinsic_height()
            } else {
                0
            }
        }

        fn intrinsic_width(&self) -> i32 {
            if let Some(paintable) = self.sink_paintable.get() {
                paintable.intrinsic_width()
            } else {
                0
            }
        }

        fn intrinsic_aspect_ratio(&self) -> f64 {
            if let Some(paintable) = self.sink_paintable.get() {
                paintable.intrinsic_aspect_ratio()
            } else {
                1.0
            }
        }

        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            if let Some(image) = self.sink_paintable.get() {
                image.snapshot(snapshot, width, height);

                if let Some(animation) = self.flash_ani.get() {
                    if !matches!(animation.state(), adw::AnimationState::Playing) {
                        return;
                    }
                    let alpha = easing(animation.value());

                    let rect = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
                    let black = gdk::RGBA::new(0.0, 0.0, 0.0, alpha as f32);

                    snapshot.append_color(&black, &rect);
                }
            } else {
                snapshot.append_color(
                    &gdk::RGBA::BLACK,
                    &graphene::Rect::new(0f32, 0f32, width as f32, height as f32),
                );
            }
        }
    }
}

glib::wrapper! {
    pub struct CameraPaintable(ObjectSubclass<imp::CameraPaintable>) @implements gdk::Paintable;
}

impl Default for CameraPaintable {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl CameraPaintable {
    pub fn set_pipewire_element(&self, element: gst::Element) {
        let imp = self.imp();

        let pipeline = imp.pipeline.get().unwrap();
        pipeline.set_pipewire_element(element);
    }

    pub fn close_pipeline(&self) {
        self.imp().pipeline.get().unwrap().close();
    }

    pub fn take_snapshot(&self, format: crate::PictureFormat) -> anyhow::Result<()> {
        let imp = self.imp();

        imp.pipeline.get().unwrap().take_snapshot(format)?;
        imp.flash_ani.get().unwrap().play();

        let settings = gio::Settings::new(config::APP_ID);
        if settings.boolean("play-shutter-sound") {
            self.play_shutter_sound();
        }

        Ok(())
    }

    // Start recording to the configured location
    pub fn start_recording(&self, format: crate::VideoFormat) -> anyhow::Result<()> {
        self.imp().pipeline.get().unwrap().start_recording(format)
    }

    // Stop recording if any recording was currently ongoing
    pub fn stop_recording(&self) {
        self.imp().pipeline.get().unwrap().stop_recording();
    }

    fn emit_code_detected(&self, code: &str) {
        self.emit_by_name::<()>("code-detected", &[&code]);
    }

    pub fn connect_code_detected<F: Fn(&Self, &str) + 'static>(&self, f: F) {
        self.connect_local(
            "code-detected",
            false,
            glib::clone!(@weak self as obj => @default-return None, move |args: &[glib::Value]| {
                let code = args.get(1).unwrap().get::<&str>().unwrap();
                f(&obj, code);

                None
            }),
        );
    }

    fn emit_picture_stored(&self, file: Option<&gio::File>) {
        self.emit_by_name::<()>("picture-stored", &[&file]);
    }

    fn emit_video_stored(&self, file: Option<&gio::File>) {
        self.emit_by_name::<()>("video-stored", &[&file]);
    }

    pub fn connect_picture_stored<F: Fn(&Self, Option<&gio::File>) + 'static>(&self, f: F) {
        self.connect_local(
            "picture-stored",
            false,
            glib::clone!(@weak self as obj => @default-return None, move |args: &[glib::Value]| {
                let file = args.get(1).unwrap().get::<Option<gio::File>>().unwrap();
                f(&obj, file.as_ref());

                None
            }),
        );
    }

    pub fn connect_video_stored<F: Fn(&Self, Option<&gio::File>) + 'static>(&self, f: F) {
        self.connect_local(
            "video-stored",
            false,
            glib::clone!(@weak self as obj => @default-return None, move |args: &[glib::Value]| {
                let file = args.get(1).unwrap().get::<Option<gio::File>>().unwrap();
                f(&obj, file.as_ref());

                None
            }),
        );
    }

    // HACK This is Uggly. This is called as
    // ```
    // picture.set_paintable(&paintable);
    // paintable.set_picture(&picture);
    // ```
    pub fn set_picture<W: glib::IsA<gtk::Picture>>(&self, picture: &W) {
        picture.as_ref().set_paintable(Some(self));

        let target =
            adw::CallbackAnimationTarget::new(glib::clone!(@weak self as obj => move |_value| {
                obj.invalidate_contents();
            }));
        let ani = adw::TimedAnimation::new(picture.upcast_ref(), 0.0, 1.0, 250, target);
        ani.set_easing(adw::Easing::Linear);

        self.imp().flash_ani.set(ani).unwrap();
    }

    fn play_shutter_sound(&self) {
        // If we don't hold a reference to it there is a condition race which
        // will cause the sound to play only sometimes.
        let resource = "/org/gnome/World/Snapshot/sounds/camera-shutter.wav";
        let player = gtk::MediaFile::for_resource(resource);
        player.play();

        self.imp().players.replace(Some(player));
    }
}

#[inline]
fn easing(value: f64) -> f64 {
    let value = 1.0 - (1.0 - value).powi(3);

    value * (1.0 - value) * 4.0
}
