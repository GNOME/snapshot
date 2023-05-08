// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene};

mod imp {
    use super::*;

    use once_cell::sync::OnceCell;

    use std::cell::Cell;
    use std::sync::Once;

    static ANIMATION_SINGLETON: Once = Once::new();

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::GuidelinesBin)]
    pub struct GuidelinesBin {
        #[property(get, set = Self::set_draw_guidelines, explicit_notify)]
        pub draw_guidelines: Cell<bool>,

        pub animation: OnceCell<adw::TimedAnimation>,
    }

    impl GuidelinesBin {
        fn set_draw_guidelines(&self, draw_guidelines: bool) {
            let animation = self.animation.get().unwrap();

            if draw_guidelines != self.draw_guidelines.replace(draw_guidelines)
                && ANIMATION_SINGLETON.is_completed()
            {
                animation.reset();
                animation.set_reverse(!draw_guidelines);
                animation.play();
                self.obj().notify_draw_guidelines();
            }
        }

        fn calculate_aspect_ratio(&self, aspect_ratio: f32) -> (f32, f32) {
            let (width, height) = (self.obj().width(), self.obj().height());

            if height < width && (height as f32 / width as f32) < aspect_ratio.powi(-1) {
                (aspect_ratio * height as f32, height as f32)
            } else {
                (width as f32, aspect_ratio.powi(-1) * width as f32)
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GuidelinesBin {
        const NAME: &'static str = "GuidelinesBin";
        type Type = super::GuidelinesBin;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for GuidelinesBin {
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

            let obj = self.obj();

            let target =
                adw::CallbackAnimationTarget::new(glib::clone!(@weak obj => move |_value| {
                    obj.queue_draw();
                }));
            let ani = adw::TimedAnimation::new(&*obj, 0.0, 1.0, 250, target);
            ani.set_easing(adw::Easing::EaseInQuad);

            self.animation.set(ani).unwrap();
        }
    }

    impl WidgetImpl for GuidelinesBin {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.parent_snapshot(snapshot);

            if let Some(vf) = self
                .obj()
                .child()
                .and_then(|child| child.downcast::<aperture::Viewfinder>().ok())
            {
                let aspect = vf.aspect_ratio();

                if aspect > f64::EPSILON {
                    let (width, height) = self.calculate_aspect_ratio(aspect as f32);
                    let (w_width, w_height) = (self.obj().width(), self.obj().height());

                    let animation = self.animation.get().unwrap();
                    ANIMATION_SINGLETON.call_once(|| {
                        if self.draw_guidelines.get() {
                            animation.play();
                        }
                    });

                    let alpha = if animation.state() != adw::AnimationState::Playing {
                        if !self.draw_guidelines.get() {
                            animation.value_from()
                        } else {
                            animation.value_to()
                        }
                    } else {
                        animation.value()
                    };

                    if alpha.abs() > f64::EPSILON {
                        snapshot.push_opacity(alpha);

                        let white = gdk::RGBA::new(1.0, 1.0, 1.0, 0.5);
                        let black = gdk::RGBA::new(0.0, 0.0, 0.0, 0.1);

                        let h_third = (height / 3.0).round();
                        let w_third = (width / 3.0).round();

                        let offset = graphene::Point::new(
                            (w_width as f32 - width) / 2.0,
                            (w_height as f32 - height) / 2.0,
                        );

                        let bv1 = graphene::Rect::new(w_third - 1.0, 0.0, 3.0, height);
                        let bv2 = graphene::Rect::new(2.0 * w_third - 1.0, 0.0, 3.0, height);

                        let bh1 = graphene::Rect::new(0.0, h_third - 1.0, width, 3.0);
                        let bh2 = graphene::Rect::new(0.0, 2.0 * h_third - 1.0, width, 3.0);

                        let v1 = graphene::Rect::new(w_third, 0.0, 1.0, height);
                        let v2 = graphene::Rect::new(2.0 * w_third, 0.0, 1.0, height);

                        let h1 = graphene::Rect::new(0.0, h_third, width, 1.0);
                        let h2 = graphene::Rect::new(0.0, 2.0 * h_third, width, 1.0);

                        snapshot.translate(&offset);

                        snapshot.append_color(&black, &bv1);
                        snapshot.append_color(&black, &bv2);
                        snapshot.append_color(&black, &bh1);
                        snapshot.append_color(&black, &bh2);

                        snapshot.append_color(&white, &v1);
                        snapshot.append_color(&white, &v2);
                        snapshot.append_color(&white, &h1);
                        snapshot.append_color(&white, &h2);

                        snapshot.pop();
                    }
                }
            }
        }
    }

    impl BinImpl for GuidelinesBin {}
}

glib::wrapper! {
    pub struct GuidelinesBin(ObjectSubclass<imp::GuidelinesBin>)
        @extends gtk::Widget, adw::Bin;
}

impl Default for GuidelinesBin {
    fn default() -> Self {
        glib::Object::new()
    }
}
