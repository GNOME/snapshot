use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};

mod imp {
    use super::*;

    use once_cell::sync::{Lazy, OnceCell};

    #[derive(Debug, Default)]
    pub struct GalleryPicture {
        pub file: OnceCell<gio::File>,
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
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::builder::<gio::File>("file")
                    .construct_only()
                    .readwrite()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "file" => self.obj().file().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "file" => self.file.set(value.get().unwrap()).unwrap(),
                _ => unimplemented!(),
            };
        }
        fn constructed(&self) {
            self.parent_constructed();

            let widget = self.obj();

            self.picture.set_parent(&*widget);

            let file = widget.file();

            let ctx = glib::MainContext::default();
            ctx.spawn_local(glib::clone!(@weak file, @weak widget => async move {
                if let Err(err) = widget.load_texture(file).await {
                    log::error!("Could not set picture {err}");
                }
            }));
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
    pub fn new(file: &gio::File) -> Self {
        glib::Object::new(&[("file", file)])
    }

    pub fn file(&self) -> &gio::File {
        self.imp().file.get().unwrap()
    }

    pub fn texture(&self) -> Option<&gdk::Texture> {
        self.imp().texture.get()
    }

    pub fn picture(&self) -> &gtk::Picture {
        &self.imp().picture
    }

    async fn load_texture(&self, file: gio::File) -> anyhow::Result<()> {
        let imp = self.imp();
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
}
