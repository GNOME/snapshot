// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

const BORDER_WIDTH: f32 = 2.0;

mod imp {
    use super::*;

    use once_cell::sync::OnceCell;
    use std::cell::RefCell;

    #[derive(Debug, Default)]
    pub struct GalleryButton {
        pub gallery: RefCell<crate::Gallery>,

        pub size_ani: OnceCell<adw::TimedAnimation>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GalleryButton {
        const NAME: &'static str = "GalleryButton";
        type Type = super::GalleryButton;
        type ParentType = gtk::Button;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("gallerybutton");
        }
    }

    impl ObjectImpl for GalleryButton {
        fn constructed(&self) {
            self.parent_constructed();

            let widget = self.obj();

            let target =
                adw::CallbackAnimationTarget::new(glib::clone!(@weak widget => move |_value| {
                    widget.queue_draw();
                }));
            let ani = adw::TimedAnimation::new(&*widget, 0.0, 1.0, 250, &target);
            self.size_ani.set(ani).unwrap();
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

            let images = self.gallery.borrow().images();
            let Some(foreground) = images.first().and_then(|x| x.texture()) else { return; };

            // We draw the border at full size if we already had a previous
            // image otherwise at the size of the current image.
            let border_radius = if let Some(background) = images.get(1).and_then(|x| x.texture()) {
                widget.draw_texture(snapshot, background, width, height, size);
                size
            } else {
                foreground_radius
            };

            widget.draw_texture(snapshot, &foreground, width, height, foreground_radius);

            widget.draw_border(snapshot, width, height, border_radius);
        }
    }
    impl ButtonImpl for GalleryButton {}
}

glib::wrapper! {
    pub struct GalleryButton(ObjectSubclass<imp::GalleryButton>)
        @extends gtk::Widget, gtk::Button;
}

impl Default for GalleryButton {
    fn default() -> Self {
        glib::Object::new(&[])
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
        // Rect where we clip the image to.
        let x = (width - size) / 2.0;
        let y = (height - size) / 2.0;
        let rect = graphene::Rect::new(x, y, size, size);
        let s = graphene::Size::new(size / 2.0, size / 2.0);
        let rounded = gsk::RoundedRect::new(rect, s, s, s, s);

        // We draw the texture to a bigger size to preserve the aspect ration and take the square that fits into its center.
        let t_width = texture.width() as f32;
        let t_height = texture.height() as f32;
        let t_ratio = t_width / t_height;
        let (t_width, t_height) = if t_ratio >= 1.0 {
            (t_ratio * size, size)
        } else {
            (size, t_ratio * size)
        };
        let t_x = -(t_width - width) / 2.0;
        let t_y = -(t_height - height) / 2.0;
        let t_rect = graphene::Rect::new(t_x, t_y, t_width, t_height);

        snapshot.push_rounded_clip(&rounded);
        snapshot.append_texture(texture, &t_rect);
        snapshot.pop();
    }

    fn draw_border(&self, snapshot: &gtk::Snapshot, width: f32, height: f32, size: f32) {
        let x = (width - size) / 2.0;
        let y = (height - size) / 2.0;

        let rect = graphene::Rect::new(x, y, size, size);
        let s = graphene::Size::new(size / 2.0, size / 2.0);
        let rounded = gsk::RoundedRect::new(rect, s, s, s, s);

        let white = gdk::RGBA::WHITE;

        snapshot.append_border(&rounded, &[BORDER_WIDTH; 4], &[white; 4]);
    }

    pub fn set_gallery(&self, gallery: crate::Gallery) {
        gallery.connect_item_added(glib::clone!(@weak self as widget => move |_, _| {
            widget.animation().play();
        }));
        *self.imp().gallery.borrow_mut() = gallery;
    }

    fn animation(&self) -> &adw::TimedAnimation {
        self.imp().size_ani.get().unwrap()
    }
}
