// SPDX-License-Identifier: GPL-3.0-or-later
use std::cell::RefCell;
use std::os::unix::io::RawFd;

use gst::prelude::*;
use gtk::glib;

use crate::config;

#[derive(Clone, Copy, Default, Debug)]
enum DeviceType {
    AudioSink,
    AudioSrc,
    VideoSrc,
    #[default]
    Other,
}

#[derive(Debug, Clone, glib::Boxed)]
#[boxed_type(name = "Device")]
pub struct Device {
    pub nick: String,
    pub element: gst::Element,
    type_: DeviceType,
}

// DO NOT implement clone.
#[derive(Debug)]
pub struct DeviceProvider {
    provider: gst::DeviceProvider,
    fd: Option<RawFd>,
    devices: RefCell<Vec<Device>>,
}

impl Drop for DeviceProvider {
    fn drop(&mut self) {
        self.provider.stop();
        if let Some(raw_fd) = self.fd {
            unsafe {
                // FIXME Replace with a OwnedFd once
                // https://github.com/bilelmoussaoui/ashpd/pull/104 is merged.
                libc::close(raw_fd);
            }
        }
    }
}

impl DeviceProvider {
    /// Creates a device provider, if a file descriptor coming for the Camera
    /// portal is passed, this will only list camera devices.
    pub fn new(fd: Option<RawFd>) -> anyhow::Result<Self> {
        log::debug!("Loading PipeWire with FD: {fd:?}");

        let provider = gst::DeviceProviderFactory::by_name("pipewiredeviceprovider").unwrap();
        if let Some(fd) = fd {
            if provider.has_property("fd", Some(RawFd::static_type())) {
                provider.set_property("fd", &fd);
            }
        }
        provider.start()?;

        let provider = Self {
            provider,
            fd,
            devices: RefCell::default(),
        };

        provider.init_devices();

        Ok(provider)
    }

    pub fn init_devices(&self) {
        let devices = self
            .provider
            .devices()
            .into_iter()
            .map(From::from)
            .collect::<Vec<Device>>();

        log::debug!("Found {} devices", devices.len());

        self.devices.replace(devices);
    }

    pub fn cameras(&self) -> Vec<Device> {
        self.devices
            .borrow()
            .clone()
            .into_iter()
            .filter(|device| matches!(device.type_, DeviceType::VideoSrc))
            .collect()
    }

    pub fn mics(&self) -> Vec<Device> {
        self.devices
            .borrow()
            .clone()
            .into_iter()
            .filter(|device| matches!(device.type_, DeviceType::AudioSrc))
            .collect()
    }
}

impl From<gst::Device> for Device {
    fn from(device: gst::Device) -> Device {
        let element = device.create_element(None).unwrap();
        element.set_property("client-name", config::APP_ID);
        let nick = device.display_name().to_string();

        let type_ = match device.device_class().as_str() {
            "Video/Source" => DeviceType::VideoSrc,
            "Audio/Source" => DeviceType::AudioSrc,
            "Audio/Sink" => DeviceType::AudioSink,
            _ => DeviceType::Other,
        };

        Device {
            element,
            nick,
            type_,
        }
    }
}
