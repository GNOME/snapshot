// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene};

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Debug, Default)]
    pub struct FlashBin {
        pub flash_ani: OnceCell<adw::TimedAnimation>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FlashBin {
        const NAME: &'static str = "FlashBin";
        type Type = super::FlashBin;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for FlashBin {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak]
                obj,
                move |_value| {
                    obj.queue_draw();
                }
            ));
            let ani = adw::TimedAnimation::new(&*obj, 0.0, 1.0, 250, target);
            ani.set_easing(adw::Easing::Linear);

            self.flash_ani.set(ani).unwrap();
        }
    }

    impl WidgetImpl for FlashBin {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let w = self.obj().width() as f32;
            let h = self.obj().height() as f32;

            self.parent_snapshot(snapshot);

            let animation = self.flash_ani.get().unwrap();
            if !matches!(animation.state(), adw::AnimationState::Playing) {
                return;
            }
            let alpha = easing(animation.value()) as f32;

            let rect = graphene::Rect::new(0.0, 0.0, w, h);
            let black = gdk::RGBA::new(0.0, 0.0, 0.0, alpha);

            snapshot.append_color(&black, &rect);
        }
    }

    impl BinImpl for FlashBin {}
}

glib::wrapper! {
    pub struct FlashBin(ObjectSubclass<imp::FlashBin>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible;
}

impl Default for FlashBin {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl FlashBin {
    pub fn flash(&self) {
        self.imp().flash_ani.get().unwrap().play();
    }
}

#[inline]
fn easing(value: f64) -> f64 {
    let value = 1.0 - (1.0 - value).powi(3);

    value * (1.0 - value) * 4.0
}
