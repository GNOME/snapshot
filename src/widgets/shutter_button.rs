// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use gettextrs::gettext;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::ShutterMode;

/// Magic factor: Consider a square with width = w, then inscribe a circle
/// inside it and then a square inside the circle. LAMBDA * w represents the
/// distance between the small and the big square, which is precisely the
/// translation we need for the record animation. Equals to (sqrt(2) - 1) / 2.
///
/// ```
/// assert!((LAMBDA - (2.0_f32.sqrt() / 2.0 - 0.5)).abs() < f32::EPSILON);
/// ```
const LAMBDA: f32 = 0.20710677;

const HOVER_SCALE: f64 = 1.05;
const HOVER_DURATION: u32 = 125;

mod imp {
    use std::cell::{Cell, OnceCell};

    use glib::Properties;

    use super::*;

    #[derive(Debug, Properties, Default)]
    #[properties(wrapper_type = super::ShutterButton)]
    pub struct ShutterButton {
        #[property(get, set = Self::set_shutter_mode, explicit_notify, default)]
        pub shutter_mode: Cell<ShutterMode>,
        #[property(get, set = Self::set_countdown, explicit_notify)]
        pub countdown: Cell<u32>,

        pub countdown_ani: OnceCell<adw::TimedAnimation>,
        pub record_ani: OnceCell<adw::TimedAnimation>,
        pub hover_ani: OnceCell<adw::TimedAnimation>,
        pub press_ani: OnceCell<adw::TimedAnimation>,
        /// Animation to play when we switch from picture to recording mode. At
        /// 1.0 we draw the button red.
        pub mode_ani: OnceCell<adw::TimedAnimation>,
    }

    impl ShutterButton {
        pub fn set_countdown(&self, countdown: u32) {
            if countdown != self.countdown.replace(countdown) {
                self.obj().notify_countdown();
            }
        }

        pub fn set_shutter_mode(&self, shutter_mode: ShutterMode) {
            let widget = self.obj();
            if shutter_mode != self.shutter_mode.replace(shutter_mode) {
                let mode_ani = widget.mode_ani();
                let record_ani = widget.record_ani();
                let mode_from = widget.mode_ani().value();
                let record_from = widget.record_ani().value();

                match shutter_mode {
                    ShutterMode::Picture => {
                        mode_ani.set_value_to(0.0);
                        mode_ani.set_value_from(mode_from);
                        mode_ani.play();

                        record_ani.set_value_from(record_from);
                        record_ani.set_value_to(0.0);
                        record_ani.play();

                        widget.set_tooltip_text(Some(&gettext("Take Picture")));
                    }
                    ShutterMode::Video => {
                        mode_ani.set_value_to(1.0);
                        mode_ani.set_value_from(mode_from);
                        mode_ani.play();

                        record_ani.set_value_from(record_from);
                        record_ani.set_value_to(0.0);
                        record_ani.play();

                        widget.set_tooltip_text(Some(&gettext("Start Recording")));
                    }
                    ShutterMode::Recording => {
                        mode_ani.set_value_to(1.0);
                        mode_ani.set_value_from(mode_from);
                        mode_ani.play();

                        record_ani.set_value_from(record_from);
                        record_ani.set_value_to(1.0);
                        record_ani.play();

                        widget.set_tooltip_text(Some(&gettext("Stop Recording")));
                    }
                }

                widget.notify_shutter_mode();
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ShutterButton {
        const NAME: &'static str = "ShutterButton";
        type Type = super::ShutterButton;
        type ParentType = gtk::Button;
    }

    #[glib::derived_properties]
    impl ObjectImpl for ShutterButton {
        fn constructed(&self) {
            self.parent_constructed();

            let widget = self.obj();

            widget.add_css_class("shutterbutton");
            widget.add_css_class("flat");

            // Set a fallback tooltip
            widget.set_tooltip_text(Some(&gettext("Take Picture")));

            // Initialize animations.
            let press_target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak]
                widget,
                move |_value| {
                    widget.queue_draw();
                }
            ));
            let press_ani = adw::TimedAnimation::new(&*widget, 4.0, 8.0, 125, press_target);
            self.press_ani.set(press_ani).unwrap();

            let hover_target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak]
                widget,
                move |_value| {
                    widget.queue_draw();
                }
            ));
            let hover_ani =
                adw::TimedAnimation::new(&*widget, 1.0, HOVER_SCALE, HOVER_DURATION, hover_target);
            self.hover_ani.set(hover_ani).unwrap();

            let mode_target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak]
                widget,
                move |_value| {
                    widget.queue_draw();
                }
            ));
            let mode_ani = adw::TimedAnimation::new(&*widget, 0.0, 1.0, 250, mode_target);
            self.mode_ani.set(mode_ani).unwrap();

            let countdown_target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak]
                widget,
                move |_value| {
                    widget.queue_draw();
                }
            ));
            let countdown_ani = adw::TimedAnimation::new(&*widget, 1.0, 0.0, 250, countdown_target);
            // TODO Figure out what easing to use.
            countdown_ani.set_easing(adw::Easing::Linear);
            countdown_ani.set_follow_enable_animations_setting(false);
            self.countdown_ani.set(countdown_ani).unwrap();

            let record_target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak]
                widget,
                move |_value| {
                    widget.queue_draw();
                }
            ));
            let record_ani = adw::TimedAnimation::new(&*widget, 0.0, 0.0, 250, record_target);
            self.record_ani.set(record_ani).unwrap();
        }
    }

    impl WidgetImpl for ShutterButton {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();

            let width = widget.width();
            let height = widget.height();

            let size = width.min(height) as f32;
            let border_width = (size / 8.0).min(4.0);

            let scale = widget.hover_ani().value() as f32;
            let translation = (scale - 1.0) * size / 2.0;

            snapshot.translate(&graphene::Point::new(-translation, -translation));
            snapshot.scale(scale, scale);

            widget.draw_border(snapshot, size, border_width);
            widget.draw_play(snapshot, size, border_width);

            self.parent_snapshot(snapshot);
        }

        fn state_flags_changed(&self, old_flags: &gtk::StateFlags) {
            self.parent_state_flags_changed(old_flags);

            let obj = self.obj();

            if obj.state_flags().contains(gtk::StateFlags::ACTIVE)
                && !old_flags.contains(gtk::StateFlags::ACTIVE)
            {
                let press_ani = obj.press_ani();
                press_ani.set_value_to(8.0);
                press_ani.play();
            } else if !obj.state_flags().contains(gtk::StateFlags::ACTIVE)
                && old_flags.contains(gtk::StateFlags::ACTIVE)
            {
                let press_ani = obj.press_ani();
                press_ani.set_value_to(4.0);
                press_ani.play();
            }

            if obj.state_flags().contains(gtk::StateFlags::PRELIGHT)
                && !old_flags.contains(gtk::StateFlags::PRELIGHT)
            {
                let hover_ani = obj.hover_ani();
                let current = hover_ani.value();
                hover_ani.pause();
                hover_ani.set_value_from(current);
                hover_ani.set_value_to(HOVER_SCALE);
                hover_ani.play();
            } else if !obj.state_flags().contains(gtk::StateFlags::PRELIGHT)
                && old_flags.contains(gtk::StateFlags::PRELIGHT)
            {
                let hover_ani = obj.hover_ani();
                let current = hover_ani.value();
                hover_ani.pause();
                hover_ani.set_value_from(current);
                hover_ani.set_value_to(1.0);
                hover_ani.play();
            }
        }
    }

    impl ButtonImpl for ShutterButton {}
}

glib::wrapper! {
    pub struct ShutterButton(ObjectSubclass<imp::ShutterButton>)
        @extends gtk::Widget, gtk::Button,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible, gtk::Actionable;
}

impl Default for ShutterButton {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl ShutterButton {
    pub fn start_countdown(&self) {
        let animation = self.countdown_ani();
        animation.set_value_to(0.0);
        animation.set_duration(self.countdown() * 1000);
        animation.play();
    }

    pub fn stop_countdown(&self) {
        let animation = self.countdown_ani();
        animation.set_value_to(1.0);
        animation.set_duration(0);
        animation.play();
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

    fn press_ani(&self) -> &adw::TimedAnimation {
        self.imp().press_ani.get().unwrap()
    }

    fn hover_ani(&self) -> &adw::TimedAnimation {
        self.imp().hover_ani.get().unwrap()
    }

    fn draw_border(&self, snapshot: &gtk::Snapshot, size: f32, border_width: f32) {
        let snap = gtk::Snapshot::new();

        let countdown = self.countdown_ani().value() as f32;

        let rect = graphene::Rect::new(0.0, 0.0, size, size);
        let center = graphene::Point::new(size / 2.0, size / 2.0);

        let s = graphene::Size::new(size / 2.0, size / 2.0);
        let rounded = gsk::RoundedRect::new(rect, s, s, s, s);

        snap.push_mask(gsk::MaskMode::Alpha);

        snap.append_border(&rounded, &[border_width; 4], &[gdk::RGBA::BLACK; 4]);

        snap.pop();

        let color = self.color();

        let stop0 = gsk::ColorStop::new(countdown, color);
        let stop1 = gsk::ColorStop::new(countdown, gdk::RGBA::TRANSPARENT);

        snap.append_conic_gradient(&rect, &center, 0.0, &[stop0, stop1]);

        snap.pop(); // Pop the mask.

        // FIXME We draw the border to a texture and then we attach it to the
        // snapshot to avoid the issue discussed at
        // https://gitlab.gnome.org/GNOME/gtk/-/issues/5755
        if let Some(node) = snap.to_node() {
            let native = self.root().and_upcast::<gtk::Native>().unwrap();
            if let Some(renderer) = native.renderer() {
                let texture = renderer.render_texture(node, Some(&rect));

                snapshot.append_texture(&texture, &rect);
            }
        }
    }

    fn draw_play(&self, snapshot: &gtk::Snapshot, size: f32, border_width: f32) {
        let imp = self.imp();

        // When we want to transform the button to a square, we don't use the
        // press animation, otherwise it looks janky as its size gets smaller
        // due to the record animation but later it gets bigger due to the press
        // animation playing in reverse.
        let gap = if matches!(imp.shutter_mode.get(), crate::ShutterMode::Picture)
            || self.countdown() > 0
        {
            self.press_ani().value() as f32
        } else {
            4.0
        };
        let record = self.record_ani().value() as f32;

        let center = size / 2.0;
        snapshot.save();
        snapshot.translate(&graphene::Point::new(center, center));
        snapshot.rotate(-90.0 * record);

        let initial_r_size = size - 2.0 * gap - 2.0 * border_width;
        let x = gap + border_width + initial_r_size * LAMBDA * record;
        let y = x;
        let rect_size = size - 2.0 * x;
        let rect = graphene::Rect::new(x - center, y - center, rect_size, rect_size);
        let big_rect = graphene::Rect::new(-center, -center, size, size);

        let border_radius = (1.0 - record * 0.7) * rect_size / 2.0;
        let s = graphene::Size::new(border_radius, border_radius);
        let rounded = gsk::RoundedRect::new(rect, s, s, s, s);

        let from_color = self.color();
        // This is Red 3 from the palette.
        let to_color = gdk::RGBA::new(0.8784314, 0.14117648, 0.105882354, 1.0);
        let t = self.mode_ani().value() as f32;
        let color = color_lerp(t, from_color, to_color);
        snapshot.push_rounded_clip(&rounded);
        // We color on a bigger rect than what we clipped.
        snapshot.append_color(&color, &big_rect);
        snapshot.pop();

        // We have to undo the transformations, otherwise the focus ring goes
        // out of place.
        snapshot.restore();
    }
}

#[inline]
fn lerp(t: f32, from: f32, to: f32) -> f32 {
    to * t + (1.0 - t) * from
}

/// Does interpolation between two colors, preserves the alpha of the original.
fn color_lerp(t: f32, from: gdk::RGBA, to: gdk::RGBA) -> gdk::RGBA {
    gdk::RGBA::new(
        lerp(t, from.red(), to.red()),
        lerp(t, from.green(), to.green()),
        lerp(t, from.blue(), to.blue()),
        from.alpha(),
    )
}
