// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::{gio, glib};

use crate::config;

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Snapshot/ui/preferences_window.ui")]
    pub struct PreferencesWindow {
        settings: OnceCell<gio::Settings>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesWindow {
        const NAME: &'static str = "PreferencesWindow";
        type Type = super::PreferencesWindow;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PreferencesWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let settings = gio::Settings::new(config::APP_ID);

            let action_group = gio::SimpleActionGroup::new();

            let play_shutter_sound = settings.create_action("play-shutter-sound");
            action_group.add_action(&play_shutter_sound);
            let show_composition_guidelines = settings.create_action("show-composition-guidelines");
            action_group.add_action(&show_composition_guidelines);

            self.obj()
                .insert_action_group("preferences-window", Some(&action_group));

            self.settings.set(settings).unwrap();
        }
    }

    impl WidgetImpl for PreferencesWindow {}
    impl AdwDialogImpl for PreferencesWindow {}
    impl PreferencesDialogImpl for PreferencesWindow {}
}

glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog;
}

impl Default for PreferencesWindow {
    fn default() -> Self {
        glib::Object::new()
    }
}
