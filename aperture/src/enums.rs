// SPDX-License-Identifier: GPL-3.0-or-later
use gtk::glib;

#[derive(Default, Debug, Copy, Clone, glib::Enum, PartialEq)]
#[enum_type(name = "ApertureViewfinderState")]
pub enum ViewfinderState {
    #[default]
    Loading,
    Ready,
    NoCameras,
    Error,
}

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
            "CODE-128" => Self::Code93,
            "PDF417" => Self::Pdf417,
            _ => Self::Unknown,
        }
    }
}
