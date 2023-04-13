// SPDX-License-Identifier: GPL-3.0-or-later
use gtk::glib;

#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::ErrorDomain)]
#[error_domain(name = "CaptureError")]
pub enum CaptureError {
    RecordingInProgress,
    StopRecordingInProgress,
    SnapshotInProgress,
    NoRecordingToStop,
    CameraDisconnected,
    Interrupted,
    NotReady,
}

impl std::error::Error for CaptureError {}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotReady => f.write_str("The viewfinder is not in the READY state"),
            Self::RecordingInProgress => f.write_str("Operation in progress: Video recording"),
            Self::SnapshotInProgress => f.write_str("Operation in progress: Take Picture"),
            Self::NoRecordingToStop => f.write_str("There is no recording to stop"),
            Self::StopRecordingInProgress => f.write_str("Operation in progress: Stop recording"),
            _ => f.write_str(&format!("{self:?}")),
        }
    }
}
