// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::{gio, glib};

use once_cell::sync::Lazy;

static ATTRIBUTES: Lazy<String> = Lazy::new(|| {
    format!(
        "{},{},{}",
        gio::FILE_ATTRIBUTE_STANDARD_NAME,
        gio::FILE_ATTRIBUTE_TIME_CREATED,
        gio::FILE_ATTRIBUTE_TIME_CREATED_USEC
    )
});

mod imp {
    use super::*;

    use glib::Properties;
    use once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/org/gnome/World/Snapshot/ui/gallery.ui")]
    #[properties(wrapper_type = super::Gallery)]
    pub struct Gallery {
        #[template_child]
        pub child: TemplateChild<gtk::Widget>,
        #[template_child]
        pub carousel: TemplateChild<adw::Carousel>,

        #[property(get = Self::progress, explicit_notify)]
        pub progress: Cell<f64>,

        pub images: RefCell<Vec<crate::GalleryItem>>,
    }

    impl Gallery {
        fn progress(&self) -> f64 {
            self.carousel.progress()
        }
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
                    let n_pages = carousel.n_pages();

                    // Suppose we have 2 pages. We add an epsilon to make sure
                    // that a rounding error 0.99999... = 1 still can scroll to
                    // the right. 0.0000...1, should also allow going to the
                    // right. We sanitize the values of the scroll, so
                    // scroll_to(-1) or scroll_to(n_items) are a non-issue.
                    obj.action_set_enabled("gallery.previous", progress + f64::EPSILON >= 1.0);
                    obj.action_set_enabled("gallery.next", progress + 2.0 <= n_pages as f64 + f64::EPSILON);

                    obj.notify("progress");

                    let index = progress as i32;
                    let last_pos = n_pages as i32 - 1;

                    if n_pages > 0 {
                        let current = carousel
                            .nth_page(index.clamp(0, last_pos) as u32)
                            .downcast::<crate::GalleryItem>().unwrap();
                        if !current.started_loading() {
                            current.start_loading();
                        }
                    }

                    if n_pages > 1 {
                        let next = carousel
                            .nth_page((index + 1).clamp(0, last_pos) as u32)
                            .downcast::<crate::GalleryItem>().unwrap();
                        if !next.started_loading() {
                            next.start_loading();
                        }
                    }

                    if index > 0 {
                        let previous = carousel
                            .nth_page((index - 1).clamp(0, last_pos) as u32)
                            .downcast::<crate::GalleryItem>().unwrap();
                        if !previous.started_loading() {
                            previous.start_loading();
                        }
                    }
                }));

            let ctx = glib::MainContext::default();
            ctx.spawn_local(glib::clone!(@weak obj => async move {
                if let Err(err) = obj.load_items().await {
                    log::debug!("Could not load latest items: {err}");
                }
            }));
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![glib::subclass::Signal::builder("item-added")
                    .param_types([crate::GalleryItem::static_type()])
                    .build()]
            });
            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
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
        let picture = self.add_item_inner(file, true, true);
        self.emit_item_added(&picture);
    }

    pub fn add_video(&self, file: &gio::File) {
        let video = self.add_item_inner(file, true, false);
        self.emit_item_added(&video);
    }

    // We have this inner method so we can add images without emiting signals.
    // Used for `load_pictures`.
    fn add_item_inner(&self, file: &gio::File, load: bool, is_picture: bool) -> crate::GalleryItem {
        let imp = self.imp();

        let item: crate::GalleryItem = if is_picture {
            crate::GalleryPicture::new(file, load).upcast()
        } else {
            crate::GalleryVideo::new(file, load).upcast()
        };

        imp.carousel.prepend(&item);
        imp.images.borrow_mut().insert(0, item.clone());

        item
    }

    pub fn open(&self) {
        // HACK The first time we call scroll_to(0) it down't do anything unless
        // we wait till the widget is drawn. At 10ms we might still have issues.
        // See https://gitlab.gnome.org/GNOME/libadwaita/-/issues/597.
        let duration = std::time::Duration::from_millis(50);
        glib::timeout_add_local_once(
            duration,
            glib::clone!(@weak self as obj => move || {
                obj.scroll_to(0, false);
            }),
        );
        self.scroll_to(0, false);
    }

    pub fn close(&self) {
        // TODO
    }

    pub fn images(&self) -> Vec<crate::GalleryItem> {
        self.imp().images.borrow().clone()
    }

    fn emit_item_added(&self, picture: &crate::GalleryItem) {
        self.emit_by_name::<()>("item-added", &[&picture]);
    }

    pub fn connect_item_added<F: Fn(&Self, &crate::GalleryItem) + 'static>(&self, f: F) {
        self.connect_closure(
            "item-added",
            false,
            glib::closure_local!(|obj, picture| {
                f(obj, picture);
            }),
        );
    }

    fn scroll_to(&self, index: i32, animate: bool) {
        let imp = self.imp();

        // Sanitize index so it is always between 0 and (n_items - 1).
        let last_pos = (imp.carousel.n_pages() as i32 - 1).max(0);
        let picture = imp
            .carousel
            .nth_page(index.clamp(0, last_pos) as u32)
            .downcast::<crate::GalleryItem>()
            .unwrap();

        imp.carousel.scroll_to(&picture, animate);
    }

    fn next(&self) {
        let imp = self.imp();
        let index = imp.carousel.position() as i32;
        self.scroll_to(index + 1, true)
    }

    fn previous(&self) {
        let imp = self.imp();
        let index = imp.carousel.position() as i32;
        self.scroll_to(index - 1, true)
    }

    async fn open_with_system(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        let index = imp.carousel.position() as u32;
        let picture = imp
            .carousel
            .nth_page(index)
            .downcast::<crate::GalleryItem>()
            .unwrap();
        let file = picture.file();
        let launcher = gtk::FileLauncher::new(Some(&file));
        let root = self.root();
        let window = root.and_downcast_ref::<gtk::Window>();
        launcher.launch_future(window).await?;

        Ok(())
    }

    async fn load_items_in(
        &self,
        dir: &std::path::Path,
        is_picture: bool,
    ) -> anyhow::Result<Vec<(gio::File, u64, bool)>> {
        let gdir = gio::File::for_path(dir);
        let enumerator = gdir
            .enumerate_children_future(
                &ATTRIBUTES,
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                glib::Priority::default(),
            )
            .await?;

        let mut items = vec![];
        while let Ok(info) = enumerator
            .next_files_future(1, glib::Priority::default())
            .await
        {
            let Some(file_info) = info.first() else { break; };

            let name = file_info.name();
            let file = gio::File::for_path(&dir.join(&name));

            // TODO Do not add items with wrong mime type.

            // TODO Try without unwrap();
            let date_time = file_info.creation_date_time().unwrap();
            let microsecond = date_time.microsecond() as u64;
            let unix = date_time.to_unix() as u64;
            let stamp = unix * 1_000_000 + microsecond;

            items.push((file, stamp, is_picture))
        }

        Ok(items)
    }

    async fn load_items(&self) -> anyhow::Result<()> {
        let pictures_dir = crate::utils::pictures_dir()?;
        let videos_dir = crate::utils::videos_dir()?;

        let mut pictures = self.load_items_in(&pictures_dir, true).await?;
        log::debug!("Done loading {} pictures", pictures.len());

        let mut videos = self.load_items_in(&videos_dir, false).await?;
        log::debug!("Done loading {} videos", videos.len());

        pictures.append(&mut videos);
        pictures.sort_by(|(_, stamp1, _), (_, stamp2, _)| stamp1.cmp(stamp2));

        if let Some(((last, _stamp, is_picture), items)) = pictures.split_last() {
            items.iter().for_each(|(file, _stamp, is_picture)| {
                self.add_item_inner(file, false, *is_picture);
            });

            let item = self.add_item_inner(last, true, *is_picture);
            self.emit_item_added(&item);
        }

        Ok(())
    }
}
