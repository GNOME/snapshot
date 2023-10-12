// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use imp::GalleryItemPropertiesExt;

pub(crate) mod imp {
    use std::cell::Cell;
    use std::cell::OnceCell;
    use std::cell::RefCell;

    use glib::Properties;

    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::GalleryItem, ext_trait)]
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
        #[property(get, set)]
        pub loaded: Cell<bool>,
        #[property(get, set = Self::set_item, explicit_notify)]
        pub item: RefCell<Option<gtk::Widget>>,

        pub popover: OnceCell<gtk::PopoverMenu>,
    }

    impl GalleryItem {
        fn set_item(&self, item: &gtk::Widget) {
            let widget = self.obj();

            if self.item.borrow().as_ref() == Some(item) {
                return;
            }

            if let Some(old) = self.item.replace(Some(item.clone())) {
                old.unparent();
            }

            item.set_parent(&*widget);
            widget.notify_item();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GalleryItem {
        const NAME: &'static str = "GalleryItem";
        type Type = super::GalleryItem;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for GalleryItem {
        fn constructed(&self) {
            self.parent_constructed();

            let widget = self.obj();

            if widget.load() {
                widget.start_loading();
            }

            widget.set_halign(gtk::Align::Center);

            let menu = crate::utils::gallery_item_menu(widget.is_picture());

            let popover = gtk::PopoverMenu::from_model(Some(&menu));
            popover.set_has_arrow(false);
            if widget.direction() == gtk::TextDirection::Rtl {
                popover.set_halign(gtk::Align::End);
            } else {
                popover.set_halign(gtk::Align::Start);
            }

            let gesture = gtk::GestureClick::new();
            gesture.set_button(gdk::BUTTON_SECONDARY);
            gesture.connect_pressed(
                glib::clone!(@weak popover, @weak widget => move |_, _, x, y| {
                    if x > -1.0 && y > -1.0 {
                        let rectangle = gdk::Rectangle::new(x as i32, y as i32, 0, 0);
                        popover.set_pointing_to(Some(&rectangle));
                    } else {
                        popover.set_pointing_to(None);
                    }
                    popover.popup();
                }),
            );

            popover.set_parent(&*widget);
            widget.add_controller(gesture);

            self.popover.set(popover).unwrap();
        }

        fn dispose(&self) {
            self.popover.get().unwrap().unparent();
            if let Some(child) = self.item.take() {
                child.unparent();
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
            let res = if widget.is_picture() {
                widget.downcast_ref::<crate::GalleryPicture>().unwrap().load_texture().await
            } else {
                widget.downcast_ref::<crate::GalleryVideo>().unwrap().load_texture().await
            };
            if let Err(err) = res {
                log::error!("Could not load gallery item: {err}");
            } else {
                widget.set_loaded(true);
            }
        }));
    }
}
