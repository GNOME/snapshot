// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

const BORDER_WIDTH: f32 = 2.0;
const HOVER_SCALE: f64 = 1.05;

mod imp {
    use std::cell::OnceCell;
    use std::cell::RefCell;

    use glib::WeakRef;

    use super::*;

    #[derive(Debug, Default)]
    pub struct GalleryButton {
        pub gallery: RefCell<Option<WeakRef<crate::Gallery>>>,

        pub size_ani: OnceCell<adw::TimedAnimation>,
        pub hover_ani: OnceCell<adw::TimedAnimation>,

        pub foreground_bindings: OnceCell<glib::SignalGroup>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GalleryButton {
        const NAME: &'static str = "GalleryButton";
        type Type = super::GalleryButton;
        type ParentType = gtk::Button;
    }

    impl ObjectImpl for GalleryButton {
        fn constructed(&self) {
            self.parent_constructed();

            let widget = self.obj();

            let bindings = glib::SignalGroup::new::<crate::GalleryItem>();
            bindings.connect_notify_local(
                Some("loaded"),
                glib::clone!(
                    #[weak]
                    widget,
                    move |_, _| {
                        widget.animation().play();
                    }
                ),
            );
            self.foreground_bindings.set(bindings).unwrap();

            widget.add_css_class("gallerybutton");
            widget.add_css_class("flat");

            let target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak]
                widget,
                move |_value| {
                    widget.queue_draw();
                }
            ));
            let ani = adw::TimedAnimation::new(&*widget, 0.0, 1.0, 250, target);
            self.size_ani.set(ani).unwrap();

            let hover_target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak]
                widget,
                move |_value| {
                    widget.queue_draw();
                }
            ));
            let hover_ani = adw::TimedAnimation::new(&*widget, 1.0, HOVER_SCALE, 125, hover_target);
            self.hover_ani.set(hover_ani).unwrap();
        }
    }

    impl WidgetImpl for GalleryButton {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();

            let width = widget.width() as f32;
            let height = widget.height() as f32;
            let size = width.min(height);

            let value = widget.animation().value() as f32;
            let foreground_radius = value * size;

            let Some(gallery) = self.gallery.borrow().as_ref().and_then(WeakRef::upgrade) else {
                return;
            };
            let items = gallery.items();
            let Some(foreground) = items.first().and_then(|x| x.thumbnail()) else {
                return;
            };

            let scale = widget.hover_ani().value() as f32;
            let translation = (scale - 1.0) * size / 2.0;

            snapshot.translate(&graphene::Point::new(-translation, -translation));
            snapshot.scale(scale, scale);

            // We draw the border at full size if we already had a previous
            // image otherwise at the size of the current image.
            let border_radius = if let Some(background) = items.get(1).and_then(|x| x.thumbnail()) {
                if matches!(widget.animation().state(), adw::AnimationState::Playing) {
                    widget.draw_texture(snapshot, &background, width, height, size);
                }
                size
            } else {
                foreground_radius
            };

            widget.draw_texture(snapshot, &foreground, width, height, foreground_radius);
            widget.draw_border(snapshot, width, height, border_radius);

            self.parent_snapshot(snapshot);
        }

        fn state_flags_changed(&self, old_flags: &gtk::StateFlags) {
            self.parent_state_flags_changed(old_flags);

            let obj = self.obj();

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

    impl ButtonImpl for GalleryButton {}
}

glib::wrapper! {
    pub struct GalleryButton(ObjectSubclass<imp::GalleryButton>)
        @extends gtk::Widget, gtk::Button,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible, gtk::Actionable;
}

impl Default for GalleryButton {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl GalleryButton {
    fn draw_texture(
        &self,
        snapshot: &gtk::Snapshot,
        texture: &gdk::Texture,
        width: f32,
        height: f32,
        size: f32,
    ) {
        // Rect where we clip the image to. We clip slightly smaller so we don't
        // have artifacts on the borders of the border.
        let x = (width - size) / 2.0;
        let y = (height - size) / 2.0;
        let e = BORDER_WIDTH * size / width / 2.0;
        let rect = graphene::Rect::new(x + e, y + e, size - 2.0 * e, size - 2.0 * e);
        let s = graphene::Size::new(size / 2.0, size / 2.0);
        let rounded = gsk::RoundedRect::new(rect, s, s, s, s);

        // We draw the texture to a bigger size to preserve the aspect ration and take
        // the square that fits into its center.
        let t_width = texture.width() as f32;
        let t_height = texture.height() as f32;
        let t_ratio = t_width / t_height;
        let (t_width, t_height) = if t_ratio >= 1.0 {
            (t_ratio * size, size)
        } else {
            (size, size / t_ratio)
        };
        let t_x = -(t_width - width) / 2.0;
        let t_y = -(t_height - height) / 2.0;
        let t_rect = graphene::Rect::new(t_x, t_y, t_width, t_height);

        snapshot.push_rounded_clip(&rounded);
        snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Linear, &t_rect);

        snapshot.pop();
    }

    fn draw_border(&self, snapshot: &gtk::Snapshot, width: f32, height: f32, size: f32) {
        let x = (width - size) / 2.0;
        let y = (height - size) / 2.0;

        let rect = graphene::Rect::new(x, y, size, size);
        let s = graphene::Size::new(size / 2.0, size / 2.0);
        let rounded = gsk::RoundedRect::new(rect, s, s, s, s);

        let color = self.color();

        snapshot.append_border(&rounded, &[BORDER_WIDTH; 4], &[color; 4]);
    }

    pub fn set_gallery(&self, gallery: &crate::Gallery) {
        gallery.connect_item_added(glib::clone!(
            #[weak(rename_to = widget)]
            self,
            move |_, item| {
                if item.loaded() {
                    widget.animation().play();
                } else {
                    widget
                        .imp()
                        .foreground_bindings
                        .get()
                        .unwrap()
                        .set_target(Some(item));
                }
            }
        ));
        gallery.connect_item_removed(glib::clone!(
            #[weak(rename_to = widget)]
            self,
            move |_, is_last| {
                if is_last {
                    widget.animation().play();
                }
            }
        ));
        self.imp().gallery.replace(Some(gallery.downgrade()));
    }

    fn animation(&self) -> &adw::TimedAnimation {
        self.imp().size_ani.get().unwrap()
    }

    fn hover_ani(&self) -> &adw::TimedAnimation {
        self.imp().hover_ani.get().unwrap()
    }
}
