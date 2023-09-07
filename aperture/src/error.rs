// SPDX-License-Identifier: GPL-3.0-or-later
use gtk::glib;

/// Describes the possible error codes of a
/// [`DeviceProvider`][crate::DeviceProvider].
#[derive(Debug, Clone)]
pub enum ProviderError {
    MissingPlugin(&'static str),
    BoolError(glib::BoolError),
}

impl From<glib::BoolError> for ProviderError {
    fn from(err: glib::BoolError) -> Self {
        Self::BoolError(err)
    }
}

impl std::error::Error for ProviderError {}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPlugin(plugin) => write!(f, "Missing gstreamer plugin {plugin}"),
            Self::BoolError(err) => write!(f, "{err}"),
        }
    }
}

/// Describes the possible error codes of a [`Viewfinder`][crate::Viewfinder]
/// while getting a capture.
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::ErrorDomain)]
#[error_domain(name = "ApertureCaptureError")]
pub enum CaptureError {
    /// A recording is already in progress and should be stopped before starting
    /// a new recording.
    RecordingInProgress,
    /// A recoding is being stopped and should finish before starting a new
    /// recording.
    StopRecordingInProgress,
    /// A picture is being taken and should be stopped before taking more
    /// pictures.
    SnapshotInProgress,
    /// No recording was found to stop.
    NoRecordingToStop,
    /// The current active camera was disconnected during capture.
    CameraDisconnected,
    /// The [`Viewfinder`][crate::Viewfinder] is not in the
    /// [`Ready`][crate::ViewfinderState::Ready] state.
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
            Self::CameraDisconnected => f.write_str("The current camera was disconnected"),
        }
    }
}

/// Describes the possible error codes of a [`Viewfinder`][crate::Viewfinder]
/// from Pipewire.
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::ErrorDomain)]
#[error_domain(name = "AperturePipewireError")]
pub enum PipewireError {
    /// The current Pipewire version is too old to use and must be upgraded.
    OldVersion,
    /// The device provider has already been started.
    ProvidedStarted,
}

impl std::error::Error for PipewireError {}

impl std::fmt::Display for PipewireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OldVersion => f.write_str("Current pipewire version is too old"),
            Self::ProvidedStarted => f.write_str("The device provided has already been started"),
        }
    }
}
