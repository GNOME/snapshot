// SPDX-License-Identifier: GPL-3.0-or-later
#[rustfmt::skip]
mod config;
mod enums;
mod objects;
mod utils;
mod widgets;

use enums::*;
use objects::*;
use widgets::*;

use gettextrs::{gettext, LocaleCategory};
use gtk::{gio, glib};

use config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};

fn main() {
    // Initialize logger
    tracing_subscriber::fmt::init();

    // Prepare i18n
    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name(&gettext("Snapshot"));

    gtk::init().expect("Unable to start GTK");
    gst::init().expect("Failed to initalize gst");
    gstgtk4::plugin_register_static().expect("Failed to initalize gstgtk4");

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    widgets::init();
    enums::init();

    let app = crate::Application::new();
    app.run();
}
