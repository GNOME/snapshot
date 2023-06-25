use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib::{self, subclass::Signal};

use super::CameraRow;

mod imp {
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;
    use std::cell::OnceCell;

    use super::*;

    #[derive(Default, Debug, CompositeTemplate)]
    #[template(resource = "/org/gnome/Snapshot/ui/camera_controls.ui")]
    pub struct CameraControls {
        pub provider: OnceCell<aperture::DeviceProvider>,

        #[template_child]
        pub gallery_button: TemplateChild<crate::GalleryButton>,
        #[template_child]
        pub camera_menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub camera_switch_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub camera_menu_button_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub shutter_button: TemplateChild<crate::ShutterButton>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CameraControls {
        const NAME: &'static str = "CameraControls";
        type Type = super::CameraControls;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl CameraControls {
        #[template_callback]
        fn on_camera_switch_button_clicked(&self) {
            self.obj().emit_by_name::<()>("camera-switched", &[])
        }
    }

    impl ObjectImpl for CameraControls {
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().connect_orientation_notify(move |obj| {
                obj.set_margin_start(0);
                obj.set_margin_end(0);
                obj.set_margin_top(0);
                obj.set_margin_bottom(0);

                match obj.orientation() {
                    gtk::Orientation::Horizontal => {
                        obj.set_margin_start(12);
                        obj.set_margin_end(12);
                    }
                    gtk::Orientation::Vertical => {
                        obj.set_margin_top(12);
                        obj.set_margin_bottom(12);
                    }
                    _ => todo!(),
                }
            });
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("camera-switched").build()]);
            SIGNALS.as_ref()
        }
    }
    impl WidgetImpl for CameraControls {}
    impl BoxImpl for CameraControls {}
}

glib::wrapper! {
    pub struct CameraControls(ObjectSubclass<imp::CameraControls>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Orientable;
}

impl Default for CameraControls {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl CameraControls {
    pub fn set_selection(&self, provider_selection: gtk::SingleSelection) {
        let popover = gtk::Popover::new();
        popover.add_css_class("menu");

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let camera_row = CameraRow::default();

            item.set_child(Some(&camera_row));
        });
        factory.connect_bind(glib::clone!(@weak provider_selection => move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                let child = item.child().unwrap();
                let row = child.downcast_ref::<CameraRow>().unwrap();

                let item = item.item().and_downcast::<aperture::Camera>().unwrap();
                row.set_item(&item);

                provider_selection.connect_selected_item_notify(glib::clone!(@weak row, @weak item => move |selection| {
                    if let Some(selected_item) = selection.selected_item() {
                        row.set_selected(selected_item == item);
                    } else {
                        row.set_selected(false);
                    }
                }));
            }));

        let list_view = gtk::ListView::new(Some(provider_selection.clone()), Some(factory));

        popover.set_child(Some(&list_view));

        provider_selection.connect_selected_item_notify(glib::clone!(@weak popover => move |_| {
            popover.popdown();
        }));

        self.imp().camera_menu_button.set_popover(Some(&popover));
    }

    pub fn connect_camera_switched<F: Fn(&Self) + 'static>(&self, f: F) {
        self.connect_closure(
            "camera-switched",
            false,
            glib::closure_local!(|obj| {
                f(obj);
            }),
        );
    }

    pub fn set_countdown(&self, countdown: u32) {
        self.imp().shutter_button.set_countdown(countdown);
    }

    pub fn start_countdown(&self) {
        self.imp().shutter_button.start_countdown();
    }

    pub fn stop_countdown(&self) {
        self.imp().shutter_button.stop_countdown();
    }

    pub fn shutter_mode(&self) -> crate::ShutterMode {
        self.imp().shutter_button.shutter_mode()
    }

    pub fn set_shutter_mode(&self, shutter_mode: crate::ShutterMode) {
        self.imp().shutter_button.set_shutter_mode(shutter_mode);
    }

    pub fn set_gallery(&self, gallery: &crate::Gallery) {
        self.imp().gallery_button.set_gallery(&gallery);
    }

    pub fn update_visible_camera_button(&self, n_cameras: u32) {
        let imp = self.imp();
        // NOTE We have a stack with an empty bin so that hiding the button does
        // not ruin the layout.
        match n_cameras {
            0 | 1 => imp
                .camera_menu_button_stack
                .set_visible_child_name("fake-widget"),
            2 => imp
                .camera_menu_button_stack
                .set_visible_child(&imp.camera_switch_button.get()),
            _ => imp
                .camera_menu_button_stack
                .set_visible_child(&imp.camera_menu_button.get()),
        }
    }
}
