// SPDX-License-Identifier: GPL-3.0-or-later
use gtk::glib;
use gtk::prelude::*;

/// Enum representing the org.gnome.World.Snapshot.PictureFormat enum defined in
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

    pub fn to_translatable_string(&self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
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

/// Enum representing the org.gnome.World.Snapshot.VideoFormat enum defined in
/// the gschema.
#[derive(Default, Debug, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum VideoFormat {
    H264Mp4,
    H265Mp4,
    Vp8Webm,
    #[default]
    TheoraOgg,
}

impl VideoFormat {
    pub fn as_str(&self) -> &str {
        match self {
            Self::H264Mp4 => "mp4",
            Self::H265Mp4 => "mp4",
            Self::Vp8Webm => "webm",
            Self::TheoraOgg => "ogg",
        }
    }
}

impl From<i32> for VideoFormat {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::H264Mp4,
            1 => Self::H265Mp4,
            2 => Self::Vp8Webm,
            3 => Self::TheoraOgg,
            _ => Self::default(),
        }
    }
}

/// Enum representing the org.gnome.World.Snapshot.CaptureMode enum defined in
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
