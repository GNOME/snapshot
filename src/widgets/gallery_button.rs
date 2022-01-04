// SPDX-License-Identifier: GPL-3.0-or-later
use std::f64::consts::PI;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene};

mod imp {
    use super::*;

    use std::cell::RefCell;

    #[derive(Debug, Default)]
    pub struct GalleryButton {
        pub gallery: RefCell<crate::Gallery>,

        front_surf: RefCell<Option<gdk::Paintable>>,
        back_surf: RefCell<Option<gdk::Paintable>>,
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

    impl ObjectImpl for GalleryButton {}
    impl WidgetImpl for GalleryButton {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();

            let width = widget.allocated_width() as f64;
            let height = widget.allocated_height() as f64;
            let size = f64::min(width, height) - 2.0;

            // TODO Use animation here for the tween
            let radius = 1.0 * size / 2.0;
            let mut outer_radius = radius;

            let images = self.gallery.borrow().images();
            let Some(first) = images.first() else { return; };

            if let Some(next) = images.get(1) {
                // TODO Use abs.
                if size != 1.0 {
                    outer_radius = size / 2.0;
                    self.back_surf.replace(next.paintable());
                    widget.draw_paintable(
                        snapshot,
                        &first.paintable().unwrap(),
                        width,
                        height,
                        size / 2.0,
                    );
                } else {
                    self.back_surf.take();
                }
            }

            self.front_surf.replace(first.paintable());

            widget.draw_paintable(snapshot, &first.paintable().unwrap(), width, height, radius);

            let rect = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);

            let ctx = snapshot.append_cairo(&rect);
            ctx.arc(width / 2.0, height / 2.0, outer_radius, 0.0, 2.0 * PI);
            ctx.set_line_width(1.5);
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            ctx.stroke().unwrap();
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
    fn draw_paintable(
        &self,
        snapshot: &gtk::Snapshot,
        paintable: &gdk::Paintable,
        width: f64,
        height: f64,
        _size: f64,
    ) {
        // TODO Clip the snapshot.
        paintable.snapshot(snapshot, width, height);
    }

    pub fn set_gallery(&self, gallery: crate::Gallery) {
        gallery.connect_item_added(glib::clone!(@weak self as widget => move |_, _| {
            widget.queue_draw();
        }));
        *self.imp().gallery.borrow_mut() = gallery;
    }
}
