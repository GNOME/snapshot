// SPDX-License-Identifier: GPL-3.0-or-later
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gio, glib};
use log::{debug, info};

use crate::config::{APP_ID, IS_DEVEL, PKGDATADIR, PROFILE, VERSION};

mod imp {
    use adw::subclass::prelude::*;

    use super::*;

    #[derive(Debug, Default)]
    pub struct Application;

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "Application";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {
        fn constructed(&self) {
            log::debug!("Application::constructed");
            self.parent_constructed();

            self.obj().add_main_option(
                "debug",
                glib::Char::from(b'd'),
                glib::OptionFlags::NONE,
                glib::OptionArg::None,
                &gettext("Enable debug messages"),
                None,
            );
        }
    }

    impl ApplicationImpl for Application {
        fn handle_local_options(
            &self,
            options: &glib::VariantDict,
        ) -> std::ops::ControlFlow<glib::ExitCode> {
            // Initialize logger
            let is_debug = options.lookup::<bool>("debug").unwrap().unwrap_or_default()
                || !glib::log_writer_default_would_drop(glib::LogLevel::Debug, Some("snapshot"));

            if is_debug {
                tracing_subscriber::fmt()
                    .with_max_level(tracing_subscriber::filter::LevelFilter::DEBUG)
                    .init();
            } else {
                tracing_subscriber::fmt::init();
            }

            log::debug!("Application::handle_local_options");

            self.parent_handle_local_options(options)
        }

        fn activate(&self) {
            debug!("Application::activate");
            self.parent_activate();

            let app = self.obj();

            if let Some(window) = app.active_window() {
                window.present();
                return;
            }

            let window = crate::Window::new(&app);
            window.present();
        }

        fn startup(&self) {
            info!("Snapshot ({APP_ID})");
            if IS_DEVEL {
                info!("Version: {VERSION} ({PROFILE})");
            } else {
                info!("Version: {VERSION}");
            }
            info!("Datadir: {PKGDATADIR}");
            debug!("Application::startup");
            self.parent_startup();

            aperture::init(APP_ID);

            crate::widgets::init();
            crate::enums::init();

            let app = self.obj();

            // Set icons for shell
            gtk::Window::set_default_icon_name(APP_ID);

            adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

            app.setup_gactions();
            app.setup_accels();
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl Default for Application {
    fn default() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("resource-base-path", "/org/gnome/Snapshot/")
            .build()
    }
}

impl Application {
    pub fn new() -> Self {
        Self::default()
    }

    fn setup_gactions(&self) {
        let actions = [gio::ActionEntryBuilder::new("quit")
            .activate(|app: &Self, _, _| app.quit())
            .build()];

        self.add_action_entries(actions);
    }

    // Sets up keyboard shortcuts
    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("win.preferences", &["<Control>comma"]);
        self.set_accels_for_action("window.close", &["<Ctrl>w"]);
        self.set_accels_for_action("win.take-picture", &["t"]);
        self.set_accels_for_action("win.toggle-gallery", &["<Control>g"]);
        self.set_accels_for_action("win.toggle-guidelines", &["c"]);
    }
}
