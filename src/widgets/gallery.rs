// SPDX-License-Identifier: GPL-3.0-or-later
use std::sync::LazyLock;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::CompositeTemplate;
use gtk::{gdk, gio, glib};

static ATTRIBUTES: LazyLock<String> = LazyLock::new(|| {
    [
        gio::FILE_ATTRIBUTE_STANDARD_NAME.as_str(),
        gio::FILE_ATTRIBUTE_TIME_CREATED.as_str(),
        gio::FILE_ATTRIBUTE_TIME_CREATED_USEC.as_str(),
        gio::FILE_ATTRIBUTE_TIME_MODIFIED.as_str(),
        gio::FILE_ATTRIBUTE_TIME_MODIFIED_USEC.as_str(),
    ]
    .join(",")
});

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Snapshot/ui/gallery.ui")]
    pub struct Gallery {
        #[template_child]
        pub sliding_view: TemplateChild<crate::SlidingView>,
        #[template_child]
        pub open_external: TemplateChild<gtk::Button>,
        #[template_child]
        pub media_controls: TemplateChild<gtk::MediaControls>,
        #[template_child]
        pub menu_button: TemplateChild<gtk::MenuButton>,

        pub current_item: RefCell<Option<crate::GalleryItem>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Gallery {
        const NAME: &'static str = "Gallery";
        type Type = super::Gallery;
        type ParentType = adw::BreakpointBin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("gallery");

            // Shows an older picture (scrolls to the right)
            klass.install_action("gallery.next", None, |widget, _, _| {
                widget.next();
            });
            // Shows a newer picture (scrolls to the left)
            klass.install_action("gallery.previous", None, |widget, _, _| {
                widget.previous();
            });
            klass.install_action_async("gallery.open", None, async |widget, _, _| {
                if let Err(err) = widget.open_with_system().await {
                    log::error!("Could not open with system handler: {err}");
                }
            });
            klass.install_action("gallery.copy", None, |widget, _, _| {
                if let Err(err) = widget.copy() {
                    log::error!("Could not copy gallery item: {err}");
                }
            });
            klass.install_action_async("gallery.delete", None, async |widget, _, _| {
                if let Err(err) = widget.delete().await {
                    log::error!("Could not delete gallery item: {err}");
                }
            });

            klass.add_binding_action(
                gdk::Key::Right,
                gdk::ModifierType::NO_MODIFIER_MASK,
                "gallery.next",
            );
            klass.add_binding_action(
                gdk::Key::KP_Right,
                gdk::ModifierType::NO_MODIFIER_MASK,
                "gallery.next",
            );
            klass.add_binding_action(
                gdk::Key::Left,
                gdk::ModifierType::NO_MODIFIER_MASK,
                "gallery.previous",
            );
            klass.add_binding_action(
                gdk::Key::KP_Left,
                gdk::ModifierType::NO_MODIFIER_MASK,
                "gallery.previous",
            );

            klass.add_binding_action(
                gdk::Key::Escape,
                gdk::ModifierType::NO_MODIFIER_MASK,
                "win.toggle-gallery",
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Gallery {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.sliding_view.connect_target_page_reached(glib::clone!(
                #[weak]
                obj,
                move |sliding_view| {
                    obj.load_neighbor_pages();

                    // When deleting an item we need to check again.
                    let has_prev = sliding_view.prev_page().is_some();
                    let has_next = sliding_view.next_page().is_some();
                    obj.action_set_enabled("gallery.previous", has_prev);
                    obj.action_set_enabled("gallery.next", has_next);
                }
            ));

            self.sliding_view.connect_current_page_notify(glib::clone!(
                #[weak]
                obj,
                move |sliding_view| {
                    let imp = obj.imp();

                    if let Some(current) = sliding_view.current_page() {
                        let is_picture = current.is_picture();

                        // The tooltip is also set in gallery.ui with a default value.
                        let tooltip_text = if is_picture {
                            gettext("Open in Image Viewer")
                        } else {
                            gettext("Open in Video Player")
                        };
                        imp.open_external.set_tooltip_text(Some(&tooltip_text));

                        let menu = crate::utils::gallery_item_menu(is_picture);
                        imp.menu_button.set_menu_model(Some(&menu));
                    }

                    let has_prev = sliding_view.prev_page().is_some();
                    let has_next = sliding_view.next_page().is_some();
                    obj.action_set_enabled("gallery.previous", has_prev);
                    obj.action_set_enabled("gallery.next", has_next);

                    if let Some(old) = obj.imp().current_item.replace(sliding_view.current_page())
                        && let Some(video) = old.downcast_ref::<crate::GalleryVideo>()
                    {
                        video.pause();
                    }

                    obj.setup_media_controls();
                }
            ));

            obj.setup_media_controls();

            glib::spawn_future_local(glib::clone!(
                #[weak]
                obj,
                async move {
                    if let Err(err) = obj.load_items().await {
                        log::debug!("Could not load latest items: {err}");
                    }
                }
            ));
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
                vec![
                    glib::subclass::Signal::builder("item-added")
                        .param_types([crate::GalleryItem::static_type()])
                        .build(),
                    glib::subclass::Signal::builder("item-removed")
                        .param_types([bool::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }
    impl WidgetImpl for Gallery {}
    impl BreakpointBinImpl for Gallery {}
}

glib::wrapper! {
    pub struct Gallery(ObjectSubclass<imp::Gallery>)
        @extends gtk::Widget, adw::BreakpointBin,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible;
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

    // We have this inner method so we can add items without emitting signals.
    // Used for `load_pictures`.
    fn add_item_inner(&self, file: &gio::File, load: bool, is_picture: bool) -> crate::GalleryItem {
        let imp = self.imp();

        let item: crate::GalleryItem = if is_picture {
            crate::GalleryPicture::new(file, load).upcast()
        } else {
            crate::GalleryVideo::new(file, load).upcast()
        };

        imp.sliding_view.prepend(&item);

        item
    }

    pub fn open(&self) {
        let imp = self.imp();

        if let Some(first) = imp.sliding_view.pages().first() {
            imp.sliding_view.scroll_to_velocity(first, None);
        }
    }

    pub fn items(&self) -> Vec<crate::GalleryItem> {
        self.imp().sliding_view.pages()
    }

    fn emit_item_added(&self, picture: &crate::GalleryItem) {
        self.emit_by_name::<()>("item-added", &[&picture]);
    }

    fn emit_item_removed(&self, is_last: bool) {
        self.emit_by_name::<()>("item-removed", &[&is_last]);
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

    pub fn connect_item_removed<F: Fn(&Self, bool) + 'static>(&self, f: F) {
        self.connect_closure(
            "item-removed",
            false,
            glib::closure_local!(|obj, is_last| {
                f(obj, is_last);
            }),
        );
    }

    fn next(&self) {
        let imp = self.imp();
        if let Some(page) = imp.sliding_view.next_page() {
            imp.sliding_view.scroll_to(&page);
        }
    }

    fn previous(&self) {
        let imp = self.imp();
        if let Some(page) = imp.sliding_view.prev_page() {
            imp.sliding_view.scroll_to(&page);
        }
    }

    async fn open_with_system(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        let Some(item) = imp.sliding_view.current_page() else {
            anyhow::bail!("SlidingView does not have current page");
        };
        let file = item.file();
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
        while let Ok(files) = enumerator
            .next_files_future(10, glib::Priority::default())
            .await
        {
            if files.is_empty() {
                break;
            }

            let mut collected = files
                .into_iter()
                .map(|file_info| {
                    let name = file_info.name();
                    let file = gio::File::for_path(dir.join(name));

                    // TODO Do not add items with wrong mime type.

                    // NOTE Filesystems that do not support either creation or modified
                    // dates will get files with a random ordering.
                    let stamp = file_info
                        .creation_date_time()
                        .or(file_info.modification_date_time())
                        .map(|date_time| {
                            let microsecond = date_time.microsecond() as u64;
                            let unix = date_time.to_unix() as u64;

                            unix * 1_000_000 + microsecond
                        })
                        .unwrap_or_default();

                    (file, stamp, is_picture)
                })
                .collect::<Vec<(gio::File, u64, bool)>>();
            items.append(&mut collected);
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

    fn load_neighbor_pages(&self) {
        let sliding_view = self.imp().sliding_view.get();

        if let Some(current) = sliding_view.current_page()
            && !current.started_loading()
        {
            current.start_loading();
        }

        if let Some(next) = sliding_view.next_page()
            && !next.started_loading()
        {
            next.start_loading();
        }

        if let Some(next) = sliding_view.next_next_page()
            && !next.started_loading()
        {
            next.start_loading();
        }

        if let Some(previous) = sliding_view.prev_page()
            && !previous.started_loading()
        {
            previous.start_loading();
        }
    }

    fn copy(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        if let Some(item) = imp.sliding_view.current_page() {
            let file = item.file();
            let list = gdk::FileList::from_array(&[file]);
            let provider = gdk::ContentProvider::for_value(&list.to_value());

            self.clipboard().set_content(Some(&provider))?;

            let window = self.root().and_downcast::<crate::Window>().unwrap();
            window.send_toast(&gettext("Copied to clipboard"));

            Ok(())
        } else {
            anyhow::bail!("Sliding view does not currently have a page");
        }
    }

    fn setup_media_controls(&self) {
        let imp = self.imp();

        if let Some(video) = imp
            .sliding_view
            .current_page()
            .and_downcast_ref::<crate::GalleryVideo>()
        {
            imp.media_controls.set_visible(true);
            imp.media_controls.set_media_stream(Some(video.stream()));
        } else {
            imp.media_controls.set_visible(false);
            imp.media_controls.set_media_stream(gtk::MediaStream::NONE);
        }
    }

    async fn delete(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        if let Some(item) = imp.sliding_view.current_page() {
            let is_last = imp
                .sliding_view
                .pages()
                .first()
                .is_some_and(|page| page == &item);

            imp.sliding_view.remove(&item);

            let file = item.file();
            file.delete_future(glib::Priority::default()).await?;

            let window = self.root().and_downcast::<crate::Window>().unwrap();
            if item.is_picture() {
                window.send_toast(&gettext("Picture deleted"));
            } else {
                window.send_toast(&gettext("Video deleted"));
            }
            self.load_neighbor_pages();

            self.emit_item_removed(is_last);

            Ok(())
        } else {
            anyhow::bail!("Sliding view does not currently have a page");
        }
    }
}
