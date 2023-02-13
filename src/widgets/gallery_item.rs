use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

mod imp {
    use super::*;

    use std::cell::Cell;
    use std::cell::RefCell;

    use glib::Properties;
    use once_cell::sync::OnceCell;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::GalleryItem)]
    pub struct GalleryItem {
        #[property(get, set, construct_only)]
        pub file: OnceCell<gio::File>,
        #[property(get, set, construct_only)]
        pub load: Cell<bool>,
        #[property(get, set, construct_only)]
        pub is_picture: Cell<bool>,
        #[property(get, set, type = Option<gdk::Texture>)]
        pub thumbnail: RefCell<Option<gdk::Texture>>,
        #[property(get, set)]
        pub started_loading: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GalleryItem {
        const NAME: &'static str = "GalleryItem";
        type Type = super::GalleryItem;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for GalleryItem {
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

            if widget.load() {
                widget.start_loading();
            }
        }
    }

    impl WidgetImpl for GalleryItem {}
    impl BinImpl for GalleryItem {}
    impl GalleryItemImpl for GalleryItem {}
}

glib::wrapper! {
    pub struct GalleryItem(ObjectSubclass<imp::GalleryItem>)
        @extends gtk::Widget, adw::Bin;
}

pub trait GalleryItemImpl: WidgetImpl + BinImpl {}

unsafe impl<T: GalleryItemImpl> IsSubclassable<T> for GalleryItem {}

impl GalleryItem {
    pub fn start_loading(&self) {
        self.set_started_loading(true);
        let ctx = glib::MainContext::default();
        ctx.spawn_local(glib::clone!(@weak self as widget => async move {
            if let Err(err) = if widget.is_picture() {
                widget.downcast_ref::<crate::GalleryPicture>().unwrap().load_texture().await
            } else {
                widget.downcast_ref::<crate::GalleryVideo>().unwrap().load_texture().await
            } {
                log::error!("Could not load gallery item: {err}");
            }
        }));
    }
}
