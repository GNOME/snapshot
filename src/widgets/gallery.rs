// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::{gdk, gio, glib};

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
        pub images: RefCell<Vec<crate::GalleryPicture>>,
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

            // Shows an older picture (scrolls to the right)
            klass.install_action("gallery.next", None, move |widget, _, _| {
                widget.next();
            });
            // Shows a newer picture (scrolls to the left)
            klass.install_action("gallery.previous", None, move |widget, _, _| {
                widget.previous();
            });
            klass.install_action_async("gallery.open", None, |widget, _, _| async move {
                if let Err(err) = widget.open_with_system().await {
                    log::error!("Could not open with system handler: {err}");
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Gallery {
        fn dispose(&self) {
            self.dispose_template();
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.carousel
                .connect_position_notify(glib::clone!(@weak obj => move |carousel| {
                    let progress = carousel.progress();

                    // Suppose we have 2 pages. We add an epsilon to make sure
                    // that a rounding error 0.99999... = 1 still can scroll to
                    // the right. 0.0000...1, should also allow going to the
                    // right. We sanitize the values of the scroll, so
                    // scroll_to(-1) or scroll_to(n_items) are a none issue.
                    obj.action_set_enabled("gallery.previous", progress + f64::EPSILON >= 1.0);
                    obj.action_set_enabled("gallery.next", progress + 2.0 <= carousel.n_pages() as f64 + f64::EPSILON);

                    obj.notify("progress");
                }));
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![glib::subclass::Signal::builder("item-added")
                    .param_types([crate::GalleryPicture::static_type()])
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
        glib::Object::new()
    }
}

impl Gallery {
    pub fn add_image(&self, file: &gio::File) {
        let imp = self.imp();

        let picture = crate::GalleryPicture::new(file);
        imp.carousel.prepend(&picture);
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

    pub fn images(&self) -> Vec<crate::GalleryPicture> {
        self.imp().images.borrow().clone()
    }

    pub fn progress(&self) -> f64 {
        self.imp().carousel.progress()
    }

    fn emit_item_added(&self, picture: &crate::GalleryPicture) {
        self.emit_by_name::<()>("item-added", &[&picture]);
    }

    pub fn connect_item_added<F: Fn(&Self, &crate::GalleryPicture) + 'static>(&self, f: F) {
        self.connect_local(
            "item-added",
            false,
            glib::clone!(@weak self as obj => @default-return None, move |args: &[glib::Value]| {
                let picture = args.get(1).unwrap().get::<crate::GalleryPicture>().unwrap();
                f(&obj, &picture);

                None
            }),
        );
    }

    fn scroll_to(&self, index: i32) {
        let imp = self.imp();

        // Sanitize index so it is always between 0 and (n_items - 1).
        let last_pos = (imp.carousel.n_pages() as i32 - 1).max(0);
        let picture = imp.carousel.nth_page(index.clamp(0, last_pos) as u32);

        imp.carousel.scroll_to(&picture, true);
    }

    fn next(&self) {
        let imp = self.imp();
        let index = imp.carousel.position() as i32;
        self.scroll_to(index + 1)
    }

    fn previous(&self) {
        let imp = self.imp();
        let index = imp.carousel.position() as i32;
        self.scroll_to(index - 1)
    }

    async fn open_with_system(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        let index = imp.carousel.position() as u32;
        let picture = imp
            .carousel
            .nth_page(index)
            .downcast::<crate::GalleryPicture>()
            .unwrap();
        let file = picture.file();
        let launcher = gtk::FileLauncher::new(Some(file));
        let root = self.root();
        let window = root.and_downcast_ref::<gtk::Window>();
        launcher.launch_future(window).await?;

        Ok(())
    }
}
