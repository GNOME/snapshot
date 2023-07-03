// SPDX-License-Identifier: GPL-3.0-or-later
use crate::widgets::gallery_item::GalleryItemImpl;
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct GalleryPicture {
        pub picture: gtk::Picture,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GalleryPicture {
        const NAME: &'static str = "GalleryPicture";
        type Type = super::GalleryPicture;
        type ParentType = crate::GalleryItem;
    }

    impl ObjectImpl for GalleryPicture {
        fn constructed(&self) {
            self.parent_constructed();

            let widget = self.obj();

            widget.set_child(Some(&self.picture));

            if let Some(basename) = widget.file().basename() {
                let label = basename.display().to_string();
                self.picture
                    .update_property(&[gtk::accessible::Property::Label(&label)]);
            }
        }
    }
    impl WidgetImpl for GalleryPicture {}
    impl BinImpl for GalleryPicture {}
    impl GalleryItemImpl for GalleryPicture {}
}

glib::wrapper! {
    pub struct GalleryPicture(ObjectSubclass<imp::GalleryPicture>)
        @extends gtk::Widget, adw::Bin, crate::GalleryItem;
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
        let (sender, receiver) = futures_channel::oneshot::channel();

        let _ = std::thread::Builder::new()
            .name("Load Texture".to_string())
            .spawn(move || {
                let result = gdk::Texture::from_file(&file);
                let _ = sender.send(result);
            });

        let texture = receiver.await.unwrap()?;

        imp.picture.set_paintable(Some(&texture));
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
