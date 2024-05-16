// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use crate::widgets::gallery_item::GalleryItemImpl;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct GalleryVideo {
        pub video_player: crate::VideoPlayer,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GalleryVideo {
        const NAME: &'static str = "GalleryVideo";
        type Type = super::GalleryVideo;
        type ParentType = crate::GalleryItem;
    }

    impl ObjectImpl for GalleryVideo {}

    impl WidgetImpl for GalleryVideo {
        fn unmap(&self) {
            self.obj().pause();
            self.parent_unmap();
        }
    }

    impl BinImpl for GalleryVideo {}
    impl GalleryItemImpl for GalleryVideo {}
}

glib::wrapper! {
    pub struct GalleryVideo(ObjectSubclass<imp::GalleryVideo>)
        @extends gtk::Widget, adw::Bin, crate::GalleryItem;
}

impl GalleryVideo {
    pub fn new(file: &gio::File, load: bool) -> Self {
        glib::Object::builder()
            .property("load", load)
            .property("file", file)
            .property("is-picture", false)
            .build()
    }

    pub fn stream(&self) -> &gtk::MediaStream {
        self.imp().video_player.stream()
    }

    pub async fn load_texture(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        self.set_started_loading(true);

        let file = self.file();
        imp.video_player.set_file(&file);

        self.upcast_ref::<crate::GalleryItem>()
            .set_item(imp.video_player.upcast_ref());

        if let Some(texture) = imp.video_player.thumbnail().await {
            self.set_thumbnail(texture);
        }

        Ok(())
    }

    pub fn pause(&self) {
        self.imp().video_player.pause();
    }

    // Ugh
    fn file(&self) -> gio::File {
        self.upcast_ref::<crate::GalleryItem>().file()
    }

    fn set_started_loading(&self, value: bool) {
        self.upcast_ref::<crate::GalleryItem>()
            .set_started_loading(value);
    }

    fn set_thumbnail(&self, value: &gdk::Texture) {
        self.upcast_ref::<crate::GalleryItem>().set_thumbnail(value);
    }
}
