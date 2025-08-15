// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk, pango};

use gettextrs::gettext;

const BORDER_RADIUS: f32 = 32.0;
const BORDER_WIDTH: f32 = 4.0;
const BG_OPACITY: f32 = 0.5;
const ANIMATION_DURATION: u32 = 500;

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use glib::Properties;

    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::QrScreenBin)]
    pub struct QrScreenBin {
        #[property(get, set)]
        viewfinder: RefCell<Option<aperture::Viewfinder>>,
        #[property(get, set = Self::set_enabled, explicit_notify)]
        enabled: Cell<bool>,

        pub dim_ani: OnceCell<adw::TimedAnimation>,
    }

    impl QrScreenBin {
        fn set_enabled(&self, enabled: bool) {
            if enabled == self.enabled.replace(enabled) {
                return;
            }

            let animation = self.dim_ani.get().unwrap();
            animation.reset();
            animation.set_reverse(!enabled);
            animation.play();

            self.obj().notify_enabled();
        }

        fn draw_text(&self, snapshot: &gtk::Snapshot, y: f32) {
            let obj = self.obj();

            let w = self.obj().width() as f32;

            let layout = obj.create_pango_layout(Some(&gettext("Scan Code")));
            let mut font_description = pango::FontDescription::new();
            font_description.set_weight(pango::Weight::Semibold);
            font_description.set_size(pango::SCALE * 16);
            layout.set_font_description(Some(&font_description));

            let (_, txt_extents) = layout.pixel_extents();
            let text_width = txt_extents.width() as f32;

            let txt_x = (w - text_width) / 2.0;
            let txt_y = y + 12.0;

            let point = graphene::Point::new(txt_x, txt_y);
            snapshot.translate(&point);

            snapshot.append_layout(&layout, &gdk::RGBA::WHITE);
        }

        fn calculate_aspect_ratio(&self, aspect_ratio: f32) -> (f32, f32) {
            let (width, height) = (self.obj().width() as f32, self.obj().height() as f32);

            if height < width && aspect_ratio < width / height {
                (height * aspect_ratio, height)
            } else {
                (width, width / aspect_ratio)
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for QrScreenBin {
        const NAME: &'static str = "QrScreenBin";
        type Type = super::QrScreenBin;
        type ParentType = adw::Bin;
    }

    #[glib::derived_properties]
    impl ObjectImpl for QrScreenBin {
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
            let ani = adw::TimedAnimation::new(&*obj, 0.0, 1.0, ANIMATION_DURATION, target);

            self.dim_ani.set(ani).unwrap();
        }
    }

    impl WidgetImpl for QrScreenBin {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.parent_snapshot(snapshot);

            let obj = self.obj();

            let w = obj.width() as f32;
            let h = obj.height() as f32;

            let Some(viewfinder) = obj.viewfinder() else {
                return;
            };
            let aspect = viewfinder.aspect_ratio();

            // HACK The viewfinder extends beyond the video feed so we need to
            // compute it's actual size.
            let (inner_w, inner_h) = self.calculate_aspect_ratio(aspect as f32);

            self.parent_snapshot(snapshot);

            let Some(animation) = self.dim_ani.get() else {
                return;
            };
            let opacity = animation.value();

            let inner_min = inner_w.min(inner_h);

            // Make a square inside the viewfinder, squares are cool, easy to
            // understand.
            let s_length = inner_min;
            let s_x = (w - s_length) / 2.0;
            let s_y = (h - s_length) / 2.0;

            // The inner rounded rect.
            let l = 3.0 * s_length / 5.0;
            let x = s_x + s_length / 5.0;
            let y = s_y + s_length / 5.0;

            let rect = graphene::Rect::new(0.0, 0.0, w, h);
            let center_rect = graphene::Rect::new(x, y, l, l);
            let s = graphene::Size::new(BORDER_RADIUS, BORDER_RADIUS);
            let center_rounded = gsk::RoundedRect::new(center_rect, s, s, s, s);

            snapshot.push_opacity(opacity);

            snapshot.push_mask(gsk::MaskMode::InvertedAlpha);

            snapshot.append_border(&center_rounded, &[999.0; 4], &[gdk::RGBA::BLACK; 4]);

            snapshot.pop(); // pop mask.

            snapshot.append_color(&gdk::RGBA::new(0.0, 0.0, 0.0, BG_OPACITY), &rect);

            snapshot.pop(); // pop source.

            snapshot.append_border(&center_rounded, &[BORDER_WIDTH; 4], &[gdk::RGBA::WHITE; 4]);

            self.draw_text(snapshot, y + l);

            snapshot.pop(); // pop opacity.
        }
    }

    impl BinImpl for QrScreenBin {}
}

glib::wrapper! {
    pub struct QrScreenBin(ObjectSubclass<imp::QrScreenBin>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible;
}

impl Default for QrScreenBin {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl QrScreenBin {}
