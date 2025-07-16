// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::{gio, glib};

use crate::config;

mod imp {
    use std::cell::{Cell, OnceCell};

    use super::*;

    #[derive(Debug, Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/org/gnome/Snapshot/ui/preferences_window.ui")]
    #[properties(wrapper_type = super::PreferencesWindow)]
    pub struct PreferencesWindow {
        settings: OnceCell<gio::Settings>,

        #[property(get, set, construct_only)]
        pub is_app_recording: Cell<bool>,

        #[template_child]
        pub experimental_group: TemplateChild<adw::PreferencesGroup>,
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

    #[glib::derived_properties]
    impl ObjectImpl for PreferencesWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let settings = gio::Settings::new(config::APP_ID);

            let action_group = gio::SimpleActionGroup::new();

            let play_shutter_sound = settings.create_action("play-shutter-sound");
            action_group.add_action(&play_shutter_sound);
            let show_composition_guidelines = settings.create_action("show-composition-guidelines");
            action_group.add_action(&show_composition_guidelines);
            if !self.obj().is_app_recording() {
                let enable_audio_recording = settings.create_action("enable-audio-recording");
                action_group.add_action(&enable_audio_recording);
                let enable_hw_encoder = settings.create_action("enable-hardware-encoding");
                action_group.add_action(&enable_hw_encoder);
            }

            if aperture::is_h264_encoding_supported() {
                self.experimental_group
                    .set_visible(aperture::is_hardware_encoding_supported(
                        aperture::VideoFormat::H264Mp4,
                    ))
            } else {
                self.experimental_group
                    .set_visible(aperture::is_hardware_encoding_supported(
                        aperture::VideoFormat::Vp8Webm,
                    ))
            }
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
        @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible;
}

impl PreferencesWindow {
    pub fn new(is_recording: bool) -> Self {
        glib::Object::builder()
            .property("is-app-recording", is_recording)
            .build()
    }
}
