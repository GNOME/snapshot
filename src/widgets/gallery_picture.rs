use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};

mod imp {
    use super::*;

    use std::cell::Cell;

    use glib::Properties;
    use once_cell::sync::OnceCell;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::GalleryPicture)]
    pub struct GalleryPicture {
        #[property(get, set, construct_only)]
        pub file: OnceCell<gio::File>,
        #[property(get, set, construct_only)]
        pub load: Cell<bool>,

        pub started_loading: Cell<bool>,

        pub picture: gtk::Picture,
        pub texture: OnceCell<gdk::Texture>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GalleryPicture {
        const NAME: &'static str = "GalleryPicture";
        type Type = super::GalleryPicture;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for GalleryPicture {
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

            let widget = self.obj();

            self.picture.set_parent(&*widget);

            if widget.load() {
                widget.start_loading();
            }
        }

        fn dispose(&self) {
            self.picture.unparent();
        }
    }
    impl WidgetImpl for GalleryPicture {}
}

glib::wrapper! {
    pub struct GalleryPicture(ObjectSubclass<imp::GalleryPicture>)
        @extends gtk::Widget;
}

impl GalleryPicture {
    /// Creates a new picture for the gallery. The texture will be load at
    /// construct only if `load` is set to `true`, otherwise it will be load
    /// when we want to snapshot it.
    pub fn new(file: &gio::File, load: bool) -> Self {
        glib::Object::builder()
            .property("load", load)
            .property("file", file)
            .build()
    }

    pub fn texture(&self) -> Option<&gdk::Texture> {
        self.imp().texture.get()
    }

    pub fn picture(&self) -> &gtk::Picture {
        &self.imp().picture
    }

    pub async fn load_texture(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        imp.started_loading.set(true);

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
        imp.texture.set(texture).unwrap();

        Ok(())
    }

    pub fn start_loading(&self) {
        self.imp().started_loading.set(true);
        let ctx = glib::MainContext::default();
        ctx.spawn_local(glib::clone!(@weak self as widget => async move {
            if let Err(err) = widget.load_texture().await {
                log::error!("Could not set picture {err}");
            }
        }));
    }

    pub fn started_loading(&self) -> bool {
        self.imp().started_loading.get()
    }
}
