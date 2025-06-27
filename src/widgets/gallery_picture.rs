// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use crate::widgets::gallery_item::GalleryItemImpl;

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Debug, Default)]
    pub struct GalleryPicture {
        pub picture: OnceCell<gtk::Picture>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GalleryPicture {
        const NAME: &'static str = "GalleryPicture";
        type Type = super::GalleryPicture;
        type ParentType = crate::GalleryItem;
    }

    impl ObjectImpl for GalleryPicture {}
    impl WidgetImpl for GalleryPicture {}
    impl BinImpl for GalleryPicture {}
    impl GalleryItemImpl for GalleryPicture {}
}

glib::wrapper! {
    pub struct GalleryPicture(ObjectSubclass<imp::GalleryPicture>)
        @extends gtk::Widget, adw::Bin, crate::GalleryItem,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible;
}

impl GalleryPicture {
    /// Creates a new picture for the gallery. The texture will be load at
    /// construct only if `load` is set to `true`, otherwise it will be load
    /// when we want to snapshot it.
    pub fn new(file: &gio::File, load: bool) -> Self {
        glib::Object::builder()
            .property("load", load)
            .property("file", file)
            .property("is-picture", true)
            .build()
    }

    pub async fn load_texture(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        self.set_started_loading(true);

        let file = self.file();
        let loader = glycin::Loader::new(file);
        let image = loader.load().await?;
        let texture = image.next_frame().await?.texture();

        let picture = imp.picture.get_or_init(gtk::Picture::default);

        self.upcast_ref::<crate::GalleryItem>()
            .set_item(picture.upcast_ref());

        if let Some(basename) = self.file().basename()
            && let Some(label) = basename.to_str()
        {
            picture.update_property(&[gtk::accessible::Property::Label(label)]);
        }

        picture.set_paintable(Some(&texture));
        self.set_thumbnail(&texture);

        Ok(())
    }

    // Ugh
    fn file(&self) -> gio::File {
        self.upcast_ref::<crate::GalleryItem>().file()
    }

    pub fn started_loading(&self) -> bool {
        self.upcast_ref::<crate::GalleryItem>().started_loading()
    }

    fn set_started_loading(&self, value: bool) {
        self.upcast_ref::<crate::GalleryItem>()
            .set_started_loading(value);
    }

    fn set_thumbnail(&self, value: &gdk::Texture) {
        self.upcast_ref::<crate::GalleryItem>().set_thumbnail(value);
    }
}
