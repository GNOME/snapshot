// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use crate::widgets::gallery_item::GalleryItemImpl;

use super::video_player;

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Debug, Default)]
    pub struct GalleryVideo {
        pub video_player: OnceCell<video_player::VideoPlayer>,
    }

    impl GalleryVideo {
        pub fn video_player(&self) -> &video_player::VideoPlayer {
            self.video_player.get_or_init(|| {
                let obj = self.obj();

                let file = obj.file();

                let video_player = crate::VideoPlayer::default();
                video_player.set_file(&file);

                obj.upcast_ref::<crate::GalleryItem>()
                    .set_item(video_player.upcast_ref());

                video_player
            })
        }
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
        @extends gtk::Widget, adw::Bin, crate::GalleryItem,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible;
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
        self.imp().video_player().stream()
    }

    pub async fn load_texture(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        self.set_started_loading(true);

        let video_player = imp.video_player();

        video_player.realize();

        if let Some(texture) = video_player.thumbnail().await {
            self.set_thumbnail(texture);
        }

        Ok(())
    }

    pub fn pause(&self) {
        if let Some(video_player) = self.imp().video_player.get() {
            video_player.pause()
        }
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
