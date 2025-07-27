// SPDX-License-Identifier: GPL-3.0-or-later
use gtk::glib;

/// Describes the possible states of a [`Viewfinder`][crate::Viewfinder]. Many
/// tasks, like taking a picture, require the viewfinder to be in a particular
/// state.
#[derive(Default, Debug, Copy, Clone, glib::Enum, PartialEq)]
#[enum_type(name = "ApertureViewfinderState")]
pub enum ViewfinderState {
    /// The viewfinder is still loading.
    #[default]
    Loading,
    /// The viewfinder is ready for use.
    Ready,
    /// The viewfinder could not find any cameras to use.
    NoCameras,
    /// The viewfinder had an error and is not usable.
    Error,
}

/// Describes the possible camera locations for a [`Camera`][crate::Camera].
#[derive(Default, Debug, Copy, Clone, glib::Enum, PartialEq)]
#[repr(u32)]
#[enum_type(name = "ApertureCameraLocation")]
pub enum CameraLocation {
    /// The camera is an internal camera, facing the front.
    Front,
    /// The camera is an internal camera, facing the back.
    Back,
    /// The camera is an external camera and has no position data.
    External,
    /// The camera position is unknown.
    #[default]
    Unknown,
}

// This only covers libcamera
impl<S: AsRef<str>> From<S> for CameraLocation {
    fn from(value: S) -> Self {
        match value.as_ref() {
            "Front" | "front" | "0" => crate::CameraLocation::Front,
            "Back" | "back" | "1" => crate::CameraLocation::Back,
            "External" | "external" | "2" => crate::CameraLocation::External,
            _ => crate::CameraLocation::Unknown,
        }
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "ApertureVideoFormat")]
pub enum VideoFormat {
    #[default]
    #[enum_value(nick = "vp8webm")]
    Vp8Webm,
    #[enum_value(nick = "h264mp4")]
    H264Mp4,
}

impl VideoFormat {
    pub fn extension_as_str(&self) -> &str {
        match self {
            Self::Vp8Webm => "webm",
            Self::H264Mp4 => "mp4",
        }
    }
}

impl From<i32> for VideoFormat {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Vp8Webm,
            1 => Self::H264Mp4,
            _ => Self::default(),
        }
    }
}
