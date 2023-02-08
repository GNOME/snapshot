// SPDX-License-Identifier: GPL-3.0-or-later
use std::f64::consts::PI;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::subclass::prelude::*;
use gtk::{cairo, glib, graphene};

use crate::ShutterMode;

mod imp {
    use super::*;

    use once_cell::sync::{Lazy, OnceCell};
    use std::cell::Cell;

    #[derive(Debug)]
    pub struct ShutterButton {
        pub shutter_mode: Cell<ShutterMode>,

        pub countdown: Cell<u32>,

        pub countdown_val: Cell<f64>,
        pub record_val: Cell<f64>,
        pub press_val: Cell<f64>,
        pub mode_val: Cell<f64>,

        pub countdown_ani: OnceCell<adw::TimedAnimation>,
        pub record_ani: OnceCell<adw::TimedAnimation>,
        pub press_ani: OnceCell<adw::TimedAnimation>,
        pub mode_ani: OnceCell<adw::TimedAnimation>,
    }

    impl Default for ShutterButton {
        fn default() -> Self {
            Self {
                shutter_mode: Default::default(),

                countdown: Cell::new(0),

                countdown_val: Cell::new(1.0),
                record_val: Cell::new(0.0),
                press_val: Cell::new(3.0),
                mode_val: Cell::new(1.0),

                countdown_ani: Default::default(),
                record_ani: Default::default(),
                press_ani: Default::default(),
                mode_ani: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ShutterButton {
        const NAME: &'static str = "ShutterButton";
        type Type = super::ShutterButton;
        type ParentType = gtk::Button;
    }

    impl ObjectImpl for ShutterButton {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecEnum::builder::<ShutterMode>(
                        "shutter-mode",
                        ShutterMode::default(),
                    )
                    .readwrite()
                    .build(),
                    glib::ParamSpecInt::builder("countdown").readwrite().build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "shutter-mode" => self.shutter_mode.get().to_value(),
                "countdown" => self.obj().countdown().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "shutter-mode" => self.shutter_mode.set(value.get().unwrap()),
                "countdown" => self.obj().set_countdown(value.get().unwrap()),
                _ => unimplemented!(),
            };
        }

        fn constructed(&self) {
            self.parent_constructed();

            let widget = self.obj();

            widget.add_css_class("shutterbutton");
            widget.set_tooltip_text(Some(&gettext("Shutter Button")));

            if matches!(widget.shutter_mode(), ShutterMode::Video) {
                self.mode_val.set(0.0);
                widget.queue_draw();
            }

            // Initialize animations.
            let press_target =
                adw::CallbackAnimationTarget::new(glib::clone!(@weak widget => move |value| {
                    widget.imp().press_val.set(value);
                    widget.queue_draw();
                }));
            let press_ani = adw::TimedAnimation::new(&*widget, 0.0, 3.0, 125, &press_target);
            self.press_ani.set(press_ani).unwrap();

            let mode_target =
                adw::CallbackAnimationTarget::new(glib::clone!(@weak widget => move |value| {
                    widget.imp().mode_val.set(value);
                    widget.queue_draw();
                }));
            let mode_ani = adw::TimedAnimation::new(&*widget, 1.0, 0.0, 250, &mode_target);
            self.mode_ani.set(mode_ani).unwrap();

            let countdown_target =
                adw::CallbackAnimationTarget::new(glib::clone!(@weak widget => move |value| {
                    widget.imp().countdown_val.set(value);
                    widget.queue_draw();
                }));
            let countdown_ani =
                adw::TimedAnimation::new(&*widget, 1.0, 0.0, 250, &countdown_target);
            // TODO Figure out what easing to use.
            countdown_ani.set_easing(adw::Easing::Linear);
            self.countdown_ani.set(countdown_ani).unwrap();

            let record_target =
                adw::CallbackAnimationTarget::new(glib::clone!(@weak widget => move |value| {
                    widget.imp().record_val.set(value);
                    widget.queue_draw();
                }));
            let record_ani = adw::TimedAnimation::new(&*widget, 0.0, 0.0, 250, &record_target);
            self.record_ani.set(record_ani).unwrap();

            self.obj()
                .connect_state_flags_changed(move |obj, old_flags| {
                    if obj.state_flags().contains(gtk::StateFlags::ACTIVE)
                        && !old_flags.contains(gtk::StateFlags::ACTIVE)
                    {
                        let press_ani = obj.imp().press_ani.get().unwrap();
                        press_ani.set_value_to(6.0);
                        press_ani.play();
                    } else if !obj.state_flags().contains(gtk::StateFlags::ACTIVE)
                        && old_flags.contains(gtk::StateFlags::ACTIVE)
                    {
                        let press_ani = obj.imp().press_ani.get().unwrap();
                        press_ani.set_value_to(3.0);
                        press_ani.play();
                    }
                });
        }
    }

    impl WidgetImpl for ShutterButton {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();

            let width = widget.width() as f64;
            let height = widget.height() as f64;
            let size = width.min(height);

            let line = (size / 8.0).min(8.0);

            let color = if widget.is_sensitive() { 1.0 } else { 0.5 };
            let mode_color = self.mode_val.get() * color;
            let alpha = widget.color().alpha() as f64;

            let rect = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
            let ctx = snapshot.append_cairo(&rect);

            ctx.set_line_width(line);
            ctx.set_source_rgba(color, color, color, alpha);
            ctx.arc_negative(
                width / 2.0,
                height / 2.0,
                (size - line) / 2.0,
                1.5 * PI,
                (2.0 * self.countdown_val.get() - 1.5) * -PI,
            );
            ctx.stroke().unwrap();

            ctx.set_source_rgba(color, mode_color, mode_color, alpha);

            let record = self.record_val.get();
            let gap = if matches!(self.shutter_mode.get(), crate::ShutterMode::Picture) {
                self.press_val.get()
            } else {
                3.0
            };

            if record == 0.0 {
                ctx.arc(
                    width / 2.0,
                    height / 2.0,
                    size / 2.0 - line - gap,
                    0.0,
                    2.0 * PI,
                );
            } else {
                let gap = gap + 3.0 * record;
                let sq_radius = ((1.0 - record) * size / 2.0 - line - gap).max(3.0);
                let sq_size = (((size / 2.0 - line - gap) - sq_radius
                    + (2.0 * sq_radius.powi(2)).sqrt())
                .powi(2)
                    / 2.0)
                    .sqrt();

                rounded_square(&ctx, sq_size, sq_radius, width / 2.0, height / 2.0);
            }

            ctx.fill().unwrap();
        }
    }

    impl ButtonImpl for ShutterButton {}
}

glib::wrapper! {
    pub struct ShutterButton(ObjectSubclass<imp::ShutterButton>)
        @extends gtk::Widget, gtk::Button;
}

impl Default for ShutterButton {
    fn default() -> Self {
        glib::Object::new(&[])
    }
}

impl ShutterButton {
    pub fn start_countdown(&self) {
        self.countdown_ani().set_value_to(0.0);
        self.countdown_ani().set_duration(self.countdown() * 1000);
        self.countdown_ani().play();
    }

    pub fn stop_countdown(&self) {
        self.countdown_ani().set_value_to(1.0);
        self.countdown_ani().set_duration(0);
        self.countdown_ani().play();
    }

    fn record_ani(&self) -> &adw::TimedAnimation {
        self.imp().record_ani.get().unwrap()
    }

    fn mode_ani(&self) -> &adw::TimedAnimation {
        self.imp().mode_ani.get().unwrap()
    }

    fn countdown_ani(&self) -> &adw::TimedAnimation {
        self.imp().countdown_ani.get().unwrap()
    }

    pub fn countdown(&self) -> u32 {
        self.imp().countdown.get()
    }

    pub fn set_countdown(&self, countdown: u32) {
        if countdown != self.imp().countdown.replace(countdown) {
            self.notify("countdown")
        }
    }

    pub fn set_shutter_mode(&self, shutter_mode: ShutterMode) {
        let imp = self.imp();
        if shutter_mode != self.imp().shutter_mode.replace(shutter_mode) {
            let from = imp.mode_val.get();
            let record_from = imp.record_val.get();
            match shutter_mode {
                ShutterMode::Picture => {
                    self.mode_ani().set_value_to(1.0);
                    self.mode_ani().set_value_from(from);
                    self.mode_ani().play();

                    self.record_ani().set_value_from(record_from);
                    self.record_ani().set_value_to(0.0);
                    self.record_ani().play();
                }
                ShutterMode::Video => {
                    self.mode_ani().set_value_to(0.0);
                    self.mode_ani().set_value_from(from);
                    self.mode_ani().play();

                    self.record_ani().set_value_from(record_from);
                    self.record_ani().set_value_to(0.0);
                    self.record_ani().play();
                }
                ShutterMode::Recording => {
                    self.mode_ani().set_value_to(0.0);
                    self.mode_ani().set_value_from(from);
                    self.mode_ani().play();

                    self.record_ani().set_value_from(record_from);
                    self.record_ani().set_value_to(1.0);
                    self.record_ani().play();
                }
            }

            self.notify("shutter-mode");
        }
    }

    pub fn shutter_mode(&self) -> ShutterMode {
        self.imp().shutter_mode.get()
    }
}

fn rounded_square(ctx: &cairo::Context, size: f64, radius: f64, x: f64, y: f64) {
    let x = x - size;
    let y = y - size;
    let size = size * 2.0;

    // top right
    ctx.arc(x + size - radius, y + radius, radius, -0.5 * PI, 0.0);
    // bottom right
    ctx.arc(x + size - radius, y + size - radius, radius, 0.0, 0.5 * PI);
    // bottom left
    ctx.arc(x + radius, y + size - radius, radius, 0.5 * PI, PI);
    // top left
    ctx.arc(x + radius, y + radius, radius, PI, -0.5 * PI);
    // back to top right
    ctx.line_to(x + size - radius, y);
}
