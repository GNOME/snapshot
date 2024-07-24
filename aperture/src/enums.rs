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

/// Describes the possible code types of a scanned code detected by a
/// [`Viewfinder`][crate::Viewfinder].
#[derive(Default, Debug, Copy, Clone, glib::Enum, PartialEq)]
#[enum_type(name = "ApertureCodetype")]
pub enum CodeType {
    #[default]
    Unknown,
    Qr,
    Composite,
    Ean2,
    Ean5,
    Ean8,
    Ean13,
    UpcA,
    UpcE,
    Isbn13,
    Isbn10,
    I25,
    Databar,
    DatabarExp,
    Codabar,
    Code39,
    Code93,
    Code128,
    Pdf417,
}

impl From<&str> for CodeType {
    fn from(value: &str) -> Self {
        match value {
            "QR-Code" => Self::Qr,
            "COMPOSITE" => Self::Composite,
            "EAN-2" => Self::Ean2,
            "EAN-5" => Self::Ean5,
            "EAN-8" => Self::Ean8,
            "EAN-13" => Self::Ean13,
            "UPC-A" => Self::UpcA,
            "UPC-E" => Self::UpcE,
            "ISBN-10" => Self::Isbn10,
            "ISBN-13" => Self::Isbn13,
            "I2/5" => Self::I25,
            "DataBar" => Self::Databar,
            "DataBar-Exp" => Self::DatabarExp,
            "Codabar" => Self::Codabar,
            "CODE-39" => Self::Code39,
            "CODE-93" => Self::Code93,
            "CODE-128" => Self::Code128,
            "PDF417" => Self::Pdf417,
            _ => Self::Unknown,
        }
    }
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
