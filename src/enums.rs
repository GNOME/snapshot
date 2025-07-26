// SPDX-License-Identifier: GPL-3.0-or-later
use gettextrs::gettext;
use gtk::glib;
use gtk::prelude::*;

/// Enum representing the org.gnome.Snapshot.PictureFormat enum defined in
/// the gschema.
#[derive(Default, Debug, Copy, Clone, PartialEq, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PictureMode")]
pub enum PictureFormat {
    #[default]
    Jpeg,
}

impl PictureFormat {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Jpeg => "jpeg",
        }
    }

    pub fn translatable_string(&self) -> String {
        match self {
            // TRANSLATORS This is the image format presented in the preferences
            // window.
            Self::Jpeg => gettext("JPEG"),
        }
    }
}

impl From<i32> for PictureFormat {
    fn from(value: i32) -> Self {
        match value {
            1 => Self::Jpeg,
            _ => Self::default(),
        }
    }
}

/// Enum representing the org.gnome.Snapshot.CaptureMode enum defined in
/// the gschema.
#[derive(Default, Debug, Copy, Clone, PartialEq, glib::Enum)]
#[enum_type(name = "CaptureMode")]
#[repr(u32)]
pub enum CaptureMode {
    #[default]
    Picture,
    Video,
    QrDetection,
}

impl From<i32> for CaptureMode {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Picture,
            1 => Self::Video,
            2 => Self::QrDetection,
            _ => Self::default(),
        }
    }
}

#[derive(Default, Debug, Copy, Clone, glib::Enum, PartialEq)]
#[repr(u32)]
#[enum_type(name = "ShutterMode")]
pub enum ShutterMode {
    #[default]
    Picture,
    Video,
    Recording,
}

pub fn init() {
    PictureFormat::static_type();
}

#[derive(Default, Debug, Copy, Clone, PartialEq, glib::Enum)]
#[enum_type(name = "ControlsLayout")]
#[repr(u32)]
pub enum ControlsLayout {
    #[default]
    Default,
    DetectingCodes,
}
