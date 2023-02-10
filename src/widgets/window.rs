// SPDX-License-Identifier: GPL-3.0-or-later
use gettextrs::gettext;

use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::config::{APP_ID, PROFILE, VERSION};
use crate::Application;
use crate::CaptureMode;

mod imp {
    use super::*;

    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::CompositeTemplate;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/org/gnome/World/Snapshot/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub camera: TemplateChild<crate::Camera>,
        #[template_child]
        pub gallery: TemplateChild<crate::Gallery>,
        #[template_child]
        pub leaflet: TemplateChild<adw::Leaflet>,

        pub settings: gio::Settings,
        pub countdown_timer_id: RefCell<Option<glib::SourceId>>,

        pub recording_active: Cell<bool>,
    }

    impl Default for Window {
        fn default() -> Self {
            Self {
                camera: TemplateChild::default(),
                gallery: TemplateChild::default(),
                leaflet: TemplateChild::default(),

                settings: gio::Settings::new(APP_ID),
                countdown_timer_id: Default::default(),

                recording_active: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "Window";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action_async("win.take-picture", None, |window, _, _| async move {
                if let Err(err) = window.on_take_picture().await {
                    log::error!("Could not take picture: {err}");
                };
            });
            klass.install_action("win.about", None, move |window, _, _| {
                window.show_about_dialog();
            });
            klass.install_action("win.toggle-gallery", None, move |window, _, _| {
                let imp = window.imp();

                if imp.leaflet.visible_child().as_ref() == Some(imp.camera.upcast_ref()) {
                    imp.leaflet.set_visible_child(&*imp.gallery);
                    window.imp().gallery.open();
                } else {
                    imp.leaflet.set_visible_child(&*imp.camera);
                    window.imp().gallery.close();
                }
            });
            klass.install_action("win.preferences", None, move |window, _, _| {
                window.show_preferences_window();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // Devel Profile
            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            obj.action_set_enabled("win.toggle-gallery", false);
            self.gallery
                .connect_item_added(glib::clone!(@weak obj => move |_, _| {
                    obj.action_set_enabled("win.toggle-gallery", true);
                }));

            // Load latest window state
            obj.load_window_size();
            obj.setup_gactions();

            // TODO Ensure the initial values are set. This should be done more
            // cleanly.
            obj.set_capture_mode(obj.capture_mode());
            let duration = obj.countdown();
            self.camera.set_countdown(duration as u32);

            self.camera.set_gallery(self.gallery.get());

            self.camera.start();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {
        // Save window state on delete event
        fn close_request(&self) -> gtk::Inhibit {
            let window = self.obj();

            if let Err(err) = window.save_window_size() {
                log::warn!("Failed to save window state, {err:?}");
            }

            // Pass close request on to the parent
            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn setup_gactions(&self) {
        let countdown_action = self.imp().settings.create_action("countdown");
        self.imp().settings.connect_changed(
            Some("countdown"),
            glib::clone!(@weak self as window => move |_, _| {
                window.countdown_cancel();
                let duration = window.countdown();
                window.imp().camera.set_countdown(duration as u32);
            }),
        );
        self.add_action(&countdown_action);

        let capture_mode_action = self.imp().settings.create_action("capture-mode");
        self.imp().settings.connect_changed(
            Some("capture-mode"),
            glib::clone!(@weak self as window => move |_, _| {
                let capture_mode = window.capture_mode();
                log::debug!("Set capture mode to {capture_mode:?}");

                window.set_capture_mode(capture_mode);
            }),
        );
        self.add_action(&capture_mode_action);
    }

    fn save_window_size(&self) -> Result<(), glib::BoolError> {
        let imp = self.imp();

        let (width, height) = self.default_size();

        imp.settings.set_int("window-width", width)?;
        imp.settings.set_int("window-height", height)?;

        imp.settings
            .set_boolean("is-maximized", self.is_maximized())?;

        Ok(())
    }

    fn load_window_size(&self) {
        let imp = self.imp();

        let width = imp.settings.int("window-width");
        let height = imp.settings.int("window-height");
        let is_maximized = imp.settings.boolean("is-maximized");

        self.set_default_size(width, height);

        if is_maximized {
            self.maximize();
        }
    }

    fn show_about_dialog(&self) {
        let dialog = adw::AboutWindow::builder()
            .application_name(&gettext("Snapshot"))
            .application_icon(APP_ID)
            .license_type(gtk::License::Gpl30)
            .issue_url("https://gitlab.gnome.org/msandova/snapshot/-/issues/new")
            .version(VERSION)
            .translator_credits(&gettext("translator-credits"))
            .developer_name("Maximiliano Sandoval")
            .developers(vec!["Maximiliano Sandoval"])
            .designers(vec!["Tobias Bernard"])
            .transient_for(self)
            .modal(true)
            .build();

        dialog.present();
    }

    fn countdown_cancel(&self) {
        if let Some(source_id) = self.imp().countdown_timer_id.take() {
            source_id.remove();
        }
        self.countdown_cleanup();
    }

    fn countdown_cleanup(&self) {
        self.imp().camera.stop_countdown();
    }

    fn countdown_start(&self) {
        if self.is_countdown_active() {
            return;
        }

        let duration: i32 = self.countdown();

        self.imp().camera.set_countdown(duration as u32);
        self.imp().camera.start_countdown();

        let duration = std::time::Duration::from_secs(duration as u64);
        let countdown_timer_id = glib::timeout_add_local_once(
            duration,
            glib::clone!(@weak self as window => move || {
                window.imp().countdown_timer_id.take();
                let ctx = glib::MainContext::default();
                ctx.spawn_local(glib::clone!(@weak window => async move {
                    if let Err(err) = window.shutter_action().await {
                        log::error!("Could not take picture: {err}");
                    };
                }));
                window.countdown_cleanup();
            }),
        );
        if let Some(old_id) = self
            .imp()
            .countdown_timer_id
            .replace(Some(countdown_timer_id))
        {
            old_id.remove();
        }
    }

    async fn shutter_action(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        if matches!(self.capture_mode(), CaptureMode::Video) {
            if imp.recording_active.get() {
                // disable the button while the video is ending
                self.action_set_enabled("win.take-picture", false);
                imp.recording_active.set(false);
                imp.camera.stop_recording().await?;
                self.action_set_enabled("win.take-picture", true);
                self.set_shutter_mode(crate::ShutterMode::Video);
            } else {
                imp.recording_active.set(true);
                let format = imp.settings.enum_("video-format").into();
                imp.camera.start_recording(format).await?;
                self.set_shutter_mode(crate::ShutterMode::Recording);
            }
        } else {
            let format = imp.settings.enum_("picture-format").into();
            imp.camera.take_picture(format).await?;
        }

        Ok(())
    }

    fn countdown(&self) -> i32 {
        self.imp().settings.int("countdown")
    }

    fn is_countdown_active(&self) -> bool {
        self.imp().countdown_timer_id.borrow().is_some()
    }

    async fn on_take_picture(&self) -> anyhow::Result<()> {
        let imp = self.imp();
        if imp.recording_active.get() {
            self.shutter_action().await?;
        } else if self.countdown() > 0 {
            if self.is_countdown_active() {
                self.countdown_cancel();
            } else {
                self.countdown_start();
            }
        } else {
            self.shutter_action().await?;
        }
        Ok(())
    }

    fn capture_mode(&self) -> CaptureMode {
        self.imp().settings.enum_("capture-mode").into()
    }

    fn set_capture_mode(&self, capture_mode: CaptureMode) {
        self.countdown_cancel();

        match capture_mode {
            CaptureMode::Picture => {
                self.set_shutter_mode(crate::ShutterMode::Picture);
            }
            CaptureMode::Video => {
                self.set_shutter_mode(crate::ShutterMode::Video);
            }
        }
    }

    fn set_shutter_mode(&self, shutter_mode: crate::ShutterMode) {
        self.imp().camera.set_shutter_mode(shutter_mode);
    }

    fn show_preferences_window(&self) {
        let preferences = crate::PreferencesWindow::new(self);
        preferences.present();
    }
}
