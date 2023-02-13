use crate::widgets::gallery_item::GalleryItemImpl;
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct GalleryVideo {
        pub picture: gtk::Picture,
        pub video_player: crate::VideoPlayer,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GalleryVideo {
        const NAME: &'static str = "GalleryVideo";
        type Type = super::GalleryVideo;
        type ParentType = crate::GalleryItem;
    }

    impl ObjectImpl for GalleryVideo {
        fn constructed(&self) {
            self.parent_constructed();

            let widget = self.obj();

            let file = widget.file();
            self.video_player.set_file(&file);

            widget.set_child(Some(&self.video_player));
        }
    }
    impl WidgetImpl for GalleryVideo {}
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

    pub async fn load_texture(&self) -> anyhow::Result<()> {
        self.set_started_loading(true);

        if let Some(texture) = self.imp().video_player.thumbnail().await {
            self.set_thumbnail(texture);
        }

        Ok(())
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
