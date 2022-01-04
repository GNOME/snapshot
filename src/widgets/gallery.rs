// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::{gdk, glib};

mod imp {
    use super::*;

    use std::cell::{Cell, RefCell};

    use once_cell::sync::Lazy;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/World/Snapshot/ui/gallery.ui")]
    pub struct Gallery {
        #[template_child]
        pub overlay: TemplateChild<gtk::Widget>,
        #[template_child]
        pub carousel: TemplateChild<adw::Carousel>,

        pub progress: Cell<f64>,
        pub images: RefCell<Vec<gtk::Picture>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Gallery {
        const NAME: &'static str = "Gallery";
        type Type = super::Gallery;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_layout_manager_type::<gtk::BinLayout>();
            klass.set_css_name("gallery");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Gallery {
        fn dispose(&self) {
            self.obj().dispose_template(Self::Type::static_type());
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.carousel
                .connect_position_notify(glib::clone!(@weak obj => move |_|{
                    obj.notify("progress")
                }));
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![glib::subclass::Signal::builder("item-added")
                    .param_types([gtk::Picture::static_type()])
                    .build()]
            });
            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> =
                Lazy::new(|| vec![glib::ParamSpecInt::builder("progress").read_only().build()]);
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();
            match pspec.name() {
                "progress" => obj.progress().to_value(),
                _ => unimplemented!(),
            }
        }
    }
    impl WidgetImpl for Gallery {}
}

glib::wrapper! {
    pub struct Gallery(ObjectSubclass<imp::Gallery>)
        @extends gtk::Widget;
}

impl Default for Gallery {
    fn default() -> Self {
        glib::Object::new(&[])
    }
}

impl Gallery {
    pub fn add_image(&self, texture: &gdk::Texture) {
        let imp = self.imp();

        let picture = gtk::Picture::for_paintable(texture);
        imp.carousel.insert(&picture, 0);
        imp.images.borrow_mut().insert(0, picture.clone());

        self.emit_item_added(&picture);
    }

    pub fn open(&self) {
        let imp = self.imp();
        if let Some(first) = imp.images.borrow().first() {
            imp.carousel.scroll_to(first, false);
        }
    }

    pub fn close(&self) {
        // TODO
    }

    pub fn images(&self) -> Vec<gtk::Picture> {
        self.imp().images.borrow().clone()
    }

    pub fn progress(&self) -> f64 {
        self.imp().carousel.progress()
    }

    fn emit_item_added(&self, picture: &gtk::Picture) {
        self.emit_by_name::<()>("item-added", &[&picture]);
    }

    pub fn connect_item_added<F: Fn(&Self, &gtk::Picture) + 'static>(&self, f: F) {
        self.connect_local(
            "item-added",
            false,
            glib::clone!(@weak self as obj => @default-return None, move |args: &[glib::Value]| {
                let picture = args.get(1).unwrap().get::<gtk::Picture>().unwrap();
                f(&obj, &picture);

                None
            }),
        );
    }
}
