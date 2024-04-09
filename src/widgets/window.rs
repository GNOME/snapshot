// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use gettextrs::gettext;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::config::{APP_ID, PROFILE, VERSION};
use crate::utils;
use crate::Application;
use crate::CaptureMode;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::CompositeTemplate;

    use super::*;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/org/gnome/Snapshot/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub camera: TemplateChild<crate::Camera>,
        #[template_child]
        pub gallery: TemplateChild<crate::Gallery>,
        #[template_child]
        pub navigation_view: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub camera_page: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub gallery_page: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,

        pub settings: gio::Settings,
        pub countdown_timer_id: RefCell<Option<glib::SourceId>>,

        pub recording_active: Cell<bool>,
    }

    impl Default for Window {
        fn default() -> Self {
            Self {
                camera: TemplateChild::default(),
                gallery: TemplateChild::default(),
                navigation_view: TemplateChild::default(),
                camera_page: TemplateChild::default(),
                gallery_page: TemplateChild::default(),
                toast_overlay: TemplateChild::default(),

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
                    match window.capture_mode() {
                        CaptureMode::Picture => {
                            log::error!("Could not take picture: {err}");
                            window.send_toast(&gettext("Could not take picture"));
                        }
                        CaptureMode::Video => {
                            log::error!("Could not record video: {err}");
                            window.send_toast(&gettext("Could not record video"));
                            window.imp().recording_active.set(false);
                        }
                    }
                };
            });
            klass.install_action("win.about", None, move |window, _, _| {
                window.show_about_dialog();
            });
            klass.install_action("win.toggle-gallery", None, move |window, _, _| {
                let imp = window.imp();

                if imp
                    .navigation_view
                    .visible_page()
                    .is_some_and(|page| page == *imp.camera_page)
                {
                    imp.camera.stop_recording();
                    imp.recording_active.set(false);
                    match window.capture_mode() {
                        CaptureMode::Video => window.set_shutter_mode(crate::ShutterMode::Video),
                        CaptureMode::Picture => {
                            window.set_shutter_mode(crate::ShutterMode::Picture)
                        }
                    }
                    imp.navigation_view.push(&*imp.gallery_page);
                    window.imp().gallery.open();
                } else {
                    imp.navigation_view.pop();
                }
            });
            klass.install_action("win.toggle-guidelines", None, move |window, _, _| {
                let imp = window.imp();

                imp.camera.toggle_guidelines();
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
            self.gallery
                .connect_item_removed(glib::clone!(@weak obj => move |gallery, _| {
                    if gallery.items().is_empty() {
                        let imp = obj.imp();

                        obj.action_set_enabled("win.toggle-gallery", false);
                        imp.navigation_view.pop();
                    }
                }));

            self.camera_page
                .connect_hiding(glib::clone!(@weak obj => move |_| {
                    obj.imp().camera.stop_stream();
                }));
            self.camera_page
                .connect_showing(glib::clone!(@weak obj => move |_| {
                    obj.imp().camera.start_stream();
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

            self.navigation_view
                .connect_visible_page_notify(glib::clone!(@weak obj => move |navigation_view| {
                    let imp = obj.imp();
                    let enabled = navigation_view.visible_page().is_some_and(|page| page == *imp.camera_page);
                    obj.set_shutter_enabled(enabled);
                }));
        }
    }

    impl WidgetImpl for Window {
        fn map(&self) {
            self.parent_map();
            let camera = self.camera.get();
            glib::spawn_future_local(glib::clone!(@weak camera => async move {
                // HACK we add a small timeout to give the shell time to get the
                // windows focus, otherwise the Shell won't ask for camera
                // permission.
                glib::timeout_future(std::time::Duration::from_millis(250)).await;
                camera.start().await;
            }));
        }
    }

    impl WindowImpl for Window {
        // Save window state on delete event
        fn close_request(&self) -> glib::Propagation {
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
        @implements gio::ActionMap, gio::ActionGroup, gtk::Root;
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
        let dialog = adw::AboutDialog::builder()
            .application_name(gettext("Camera"))
            .application_icon(APP_ID)
            .license_type(gtk::License::Gpl30)
            .issue_url("https://gitlab.gnome.org/GNOME/snapshot/-/issues/new")
            .version(VERSION)
            .translator_credits(gettext("translator-credits"))
            .developer_name(gettext("The GNOME Project"))
            .developers(["Maximiliano Sandoval", "Jamie Murphy <jmurphy@gnome.org>"])
            .designers(["Tobias Bernard"])
            .debug_info(utils::debug_info())
            .build();

        dialog.present(self);
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
                glib::spawn_future_local(glib::clone!(@weak window => async move {
                    if let Err(err) = window.shutter_action().await {
                        match window.capture_mode() {
                            CaptureMode::Picture => {
                                log::error!("Could not take picture: {err}");
                                window.send_toast(&gettext("Could not take picture"));
                            }
                            CaptureMode::Video => {
                                log::error!("Could not record video: {err}");
                                window.send_toast(&gettext("Could not record video"));
                                window.imp().recording_active.set(false);
                            }
                        }
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
                //
                // TODO This is prone to errors, create start/stop_decoding functions
                // that do the correct thing.
                self.set_shutter_enabled(false);
                imp.recording_active.set(false);
                imp.camera.stop_recording();
                self.set_shutter_enabled(true);
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
        let preferences = crate::PreferencesWindow::default();
        preferences.present(self);
    }

    pub fn send_toast(&self, text: &str) {
        let toast = adw::Toast::new(text);
        self.imp().toast_overlay.add_toast(toast);
    }

    pub fn set_shutter_enabled(&self, enabled: bool) {
        self.action_set_enabled("win.take-picture", enabled);
    }
}
