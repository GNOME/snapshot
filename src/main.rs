// SPDX-License-Identifier: GPL-3.0-or-later
#[rustfmt::skip]
mod config;
mod application;
mod enums;
mod i18n;
mod utils;
mod widgets;

use application::Application;
use config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};
use enums::*;
use gettextrs::{LocaleCategory, gettext};
use gtk::prelude::*;
use gtk::{gio, glib};
use widgets::*;

fn main() -> glib::ExitCode {
    // Prepare i18n
    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name(&gettext("Camera"));

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    let app = crate::Application::new();

    app.run()
}
