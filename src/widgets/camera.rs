// SPDX-License-Identifier: GPL-3.0-or-later
use std::os::unix::io::RawFd;

use ashpd::desktop::camera;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use gtk::{prelude::*, CompositeTemplate};

use crate::{CameraRow, Device};

const PROVIDER_TIMEOUT: u64 = 2;

mod imp {
    use super::*;

    use once_cell::unsync::OnceCell;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/World/Snapshot/ui/camera.ui")]
    pub struct Camera {
        pub paintable: crate::CameraPaintable,
        pub stream_list: RefCell<gio::ListStore>,
        pub selection: gtk::SingleSelection,
        pub provider: OnceCell<crate::DeviceProvider>,
        pub listener: OnceCell<crate::WaylandListener>,

        #[template_child]
        pub gallery_button: TemplateChild<crate::GalleryButton>,
        #[template_child]
        pub camera_menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub camera_menu_button_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub shutter_button: TemplateChild<crate::ShutterButton>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Camera {
        const NAME: &'static str = "Camera";
        type Type = super::Camera;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Camera {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.paintable.set_picture(&*self.picture);

            let popover = gtk::Popover::new();
            popover.add_css_class("menu");

            let provider = crate::DeviceProvider::new();
            provider.connect_items_changed(glib::clone!(@weak obj => move |provider, _, _, _| {
                obj.update_cameras(provider);
            }));
            obj.update_cameras(&provider);

            self.selection.set_model(Some(&provider));
            let factory = gtk::SignalListItemFactory::new();
            factory.connect_setup(|_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                let camera_row = CameraRow::default();

                item.set_child(Some(&camera_row));
            });
            let selection = &self.selection;
            factory.connect_bind(glib::clone!(@weak selection => move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                let child = item.child().unwrap();
                let row = child.downcast_ref::<CameraRow>().unwrap();

                let item = item.item().unwrap().downcast::<Device>().unwrap();
                row.set_item(&item);

                selection.connect_selected_item_notify(glib::clone!(@weak row, @weak item => move |selection| {
                    if let Some(selected_item) = selection.selected_item() {
                        row.set_selected(selected_item == item);
                    } else {
                        row.set_selected(false);
                    }
                }));
            }));
            let list_view = gtk::ListView::new(Some(self.selection.clone()), Some(factory));

            popover.set_child(Some(&list_view));

            self.selection.connect_selected_item_notify(
                glib::clone!(@weak obj, @weak popover => move |selection| {
                    if let Some(selected_item) = selection.selected_item() {
                        let device = selected_item.downcast_ref::<Device>().unwrap();
                        obj.imp().paintable.set_pipewire_element(device.element());
                    }
                    popover.popdown();
                }),
            );

            self.camera_menu_button.set_popover(Some(&popover));

            self.paintable.connect_code_detected(|_, code| {
                // TODO Do a proper dialog here.
                log::debug!("Found QR code with contents: {code}");
            });

            self.paintable
                .connect_picture_stored(glib::clone!(@weak obj => move |_, _| {
                    obj.imp().shutter_button.set_sensitive(true);
                }));

            self.provider.set(provider).unwrap();

            // This spinner stops running when the device provider finds any
            // camera device.
            self.spinner.start();
            self.stack.set_visible_child_name("loading");
        }

        fn dispose(&self) {
            self.paintable.stop_recording();
            self.dispose_template();
        }
    }
    impl WidgetImpl for Camera {
        fn realize(&self) {
            self.parent_realize();

            let widget = self.obj();

            // Its better to ask for displays on realized widgets.
            let display = widget.display();
            let listener = crate::WaylandListener::new(display);
            listener
                .bind_property("transform", &widget.imp().paintable, "transform")
                .sync_create()
                .build();

            self.listener.set(listener).unwrap();
        }
    }
}

glib::wrapper! {
    pub struct Camera(ObjectSubclass<imp::Camera>)
        @extends gtk::Widget;
}

impl Default for Camera {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Camera {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stop(&self) {
        self.imp().paintable.close_pipeline();
    }

    pub async fn start(&self) {
        let provider = self.imp().provider.get().unwrap();

        let ctx = glib::MainContext::default();
        ctx.spawn_local(
            glib::clone!(@weak self as obj, @strong provider => async move {
                if let Ok(fd) = stream().await {
                    if let Err(err) = provider.set_fd(fd) {
                        log::error!("Could not use the camera portal: {err}");
                    };
                } else {
                    // FIXME Show a page explaining how to setup the permission.
                    log::warn!("Could not use the camera portal");
                }

                if let Err(err) = provider.start() {
                    obj.imp().stack.set_visible_child_name("not-found");
                    log::error!("Could not start device provider: {err}");
                }
            }),
        );

        // FIXME This is super arbitrary
        let duration = std::time::Duration::from_secs(PROVIDER_TIMEOUT);
        glib::timeout_add_local_once(
            duration,
            glib::clone!(@weak self as obj => move || {
                let imp = obj.imp();
                if imp.stack.visible_child_name().as_deref() == Some("loading") {
                    imp.spinner.stop();
                    imp.stack.set_visible_child_name("not-found");
                }
            }),
        );
    }

    pub async fn start_recording(&self, format: crate::VideoFormat) -> anyhow::Result<()> {
        self.imp().paintable.start_recording(format)?;
        Ok(())
    }

    pub fn stop_recording(&self) {
        self.imp().paintable.stop_recording();
    }

    pub async fn take_picture(&self, format: crate::PictureFormat) -> anyhow::Result<()> {
        let imp = self.imp();
        // We set sensitive = True whenever picture-stored is emited.
        imp.shutter_button.set_sensitive(false);
        imp.paintable.take_snapshot(format)
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
        let imp = self.imp();

        if matches!(shutter_mode, crate::ShutterMode::Picture) {
            imp.paintable.stop_recording();
        }
        imp.shutter_button.set_shutter_mode(shutter_mode);
    }

    pub fn set_gallery(&self, gallery: crate::Gallery) {
        let imp = self.imp();

        imp.paintable
            .connect_picture_stored(glib::clone!(@weak gallery,  => move |_, file| {
                if let Some(file) = file {
                    gallery.add_image(file);
                }
            }));
        imp.paintable
            .connect_video_stored(glib::clone!(@weak gallery,  => move |_, file| {
                if let Some(file) = file {
                    // HACK This is terrible, we should be able to emit this at
                    // the correct time.
                    let duration = std::time::Duration::from_millis(1500);
                    glib::timeout_add_local_once(
                        duration,
                        glib::clone!(@weak gallery, @strong file => move || {
                            gallery.add_video(&file);
                        }),
                    );
                }
            }));
        imp.gallery_button.set_gallery(&gallery);
    }

    fn update_cameras(&self, provider: &crate::DeviceProvider) {
        let imp = self.imp();
        imp.spinner.stop();

        let n_cameras = provider.n_items();
        if n_cameras == 0 {
            imp.stack.set_visible_child_name("not-found");
        } else {
            imp.stack.set_visible_child_name("camera");
        }

        // NOTE We have a stack with an empty bin so that hiding the button does
        // not ruin the layout.
        if n_cameras > 1 {
            imp.camera_menu_button_stack
                .set_visible_child(&imp.camera_menu_button.get());
        } else {
            imp.camera_menu_button_stack
                .set_visible_child_name("fake-widget");
        }
    }
}

async fn stream() -> ashpd::Result<RawFd> {
    let proxy = camera::Camera::new().await?;
    proxy.request_access().await?;

    proxy.open_pipe_wire_remote().await
}
