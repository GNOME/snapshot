// SPDX-License-Identifier: GPL-3.0-or-later
use gettextrs::gettext;
use gtk::glib;
use gtk::prelude::*;
use wayland_client::protocol::wl_output;

/// Enum representing the org.gnome.Snapshot.PictureFormat enum defined in
/// the gschema.
#[derive(Default, Debug, Copy, Clone, PartialEq, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PictureMode")]
pub enum PictureFormat {
    #[default]
    Png,
    Jpeg,
}

impl PictureFormat {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
        }
    }

    pub fn translatable_string(&self) -> String {
        match self {
            // TRANSLATORS This is the image format presented in the preferences
            // window.
            Self::Png => gettext("PNG"),
            // TRANSLATORS This is the image format presented in the preferences
            // window.
            Self::Jpeg => gettext("JPEG"),
        }
    }
}

impl From<i32> for PictureFormat {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Png,
            1 => Self::Jpeg,
            _ => Self::default(),
        }
    }
}

/// Enum representing the org.gnome.Snapshot.VideoFormat enum defined in
/// the gschema.
#[derive(Default, Debug, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum VideoFormat {
    #[default]
    Vp8Webm,
}

impl VideoFormat {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Vp8Webm => "webm",
        }
    }
}

impl From<i32> for VideoFormat {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Vp8Webm,
            _ => Self::default(),
        }
    }
}

/// Enum representing the org.gnome.Snapshot.CaptureMode enum defined in
/// the gschema.
#[derive(Default, Debug, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum CaptureMode {
    #[default]
    Picture,
    Video,
}

impl From<i32> for CaptureMode {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Picture,
            1 => Self::Video,
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

#[derive(Default, Debug, Copy, Clone, glib::Enum, PartialEq)]
#[repr(u32)]
#[enum_type(name = "Rotation")]
pub enum Transform {
    #[default]
    Identity,
    Rotation90,
    Rotation180,
    Rotation270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

impl From<wl_output::Transform> for Transform {
    fn from(value: wl_output::Transform) -> Self {
        match value {
            wl_output::Transform::Normal => Self::Identity,
            wl_output::Transform::_90 => Self::Rotation90,
            wl_output::Transform::_180 => Self::Rotation180,
            wl_output::Transform::_270 => Self::Rotation270,
            wl_output::Transform::Flipped => Self::Flipped,
            wl_output::Transform::Flipped90 => Self::Flipped90,
            wl_output::Transform::Flipped180 => Self::Flipped180,
            wl_output::Transform::Flipped270 => Self::Flipped270,
            _ => unreachable!(),
        }
    }
}

impl Transform {
    /// Returns (cos, sin) of the transform
    pub fn cos_sin(&self) -> (f32, f32) {
        match self {
            Self::Identity | Self::Flipped => (1.0, 0.0),
            Self::Rotation90 | Self::Flipped90 => (0.0, 1.0),
            Self::Rotation180 | Self::Flipped180 => (-1.0, 0.0),
            Self::Rotation270 | Self::Flipped270 => (0.0, -1.0),
        }
    }

    pub fn degs(&self) -> f32 {
        match self {
            Self::Identity | Self::Flipped => 0.0,
            Self::Rotation90 | Self::Flipped90 => 90.0,
            Self::Rotation180 | Self::Flipped180 => 180.0,
            Self::Rotation270 | Self::Flipped270 => 270.0,
        }
    }

    pub fn inverse(&self) -> Self {
        match self {
            Self::Identity => Self::Identity,
            Self::Rotation90 => Self::Rotation270,
            Self::Rotation180 => Self::Rotation180,
            Self::Rotation270 => Self::Rotation90,
            Self::Flipped => Self::Flipped,
            Self::Flipped90 => Self::Flipped270,
            Self::Flipped180 => Self::Flipped180,
            Self::Flipped270 => Self::Flipped90,
        }
    }

    pub fn is_flipped(&self) -> bool {
        matches!(
            self,
            Self::Flipped | Self::Flipped90 | Self::Flipped180 | Self::Flipped270
        )
    }

    pub fn as_gst_str(&self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Rotation90 => "90r",
            Self::Rotation180 => "180",
            Self::Rotation270 => "90l",
            // FIXME Support flips.
            Self::Flipped => "identity",
            Self::Flipped90 => "identity",
            Self::Flipped180 => "identity",
            Self::Flipped270 => "identity",
        }
    }
}
