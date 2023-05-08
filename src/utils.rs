// SPDX-License-Identifier: GPL-3.0-or-later
use gettextrs::gettext;
use gst::prelude::*;
use gtk::prelude::*;
use once_cell::sync::Lazy;
use std::path::PathBuf;

use anyhow::Context;
use gtk::glib;

pub fn picture_file_name(picture_format: crate::PictureFormat) -> String {
    // Alternatively check
    // https://gitlab.gnome.org/sdroege/las-workshop-2019/-/blob/master/src/pipeline.rs.
    let format = picture_format.as_str();
    if let Ok(date_time) = glib::DateTime::now_local() {
        format!(
            "{} {}.{format}",
            // TRANSLATORS  This will appear as, e.g. "Photo from 2023-05-21 11-05-59.12345.png"
            gettext("Photo from"),
            date_time.format("%Y-%m-%d %H-%M-%S.%f").unwrap()
        )
    } else {
        let rand = glib::random_int_range(0, 999999);
        // TRANSLATORS  This will appear as, e.g. "Photo 12345.png"
        format!("{} {rand}.{format}", gettext("Photo"))
    }
}

pub fn video_file_name(video_format: crate::VideoFormat) -> String {
    let format = video_format.as_str();
    if let Ok(date_time) = glib::DateTime::now_local() {
        format!(
            "{} {}.{format}",
            // TRANSLATORS  This will appear as, e.g. "Recording from 2023-05-21 11-05-59.12345.png"
            gettext("Recording from"),
            date_time.format("%Y-%m-%d %H-%M-%S.%f").unwrap()
        )
    } else {
        let rand = glib::random_int_range(0, 999999);
        // TRANSLATORS  This will appear as, e.g. "Recording 12345.png"
        format!("{} {rand}.{format}", gettext("Recording"))
    }
}

// TODO These should return a result so we stop the file saving process
// if we fail.
pub fn videos_dir() -> anyhow::Result<PathBuf> {
    let path = glib::user_special_dir(glib::UserDirectory::Videos)
        .context("Could not find XDG_VIDEOS_DIR")?
        // TODO Should this be translated? It is not expected that if the
        // user switches locales, videos now go to another folder.
        .join("Camera");

    std::fs::create_dir_all(&path)?;

    Ok(path)
}

pub fn pictures_dir() -> anyhow::Result<PathBuf> {
    let path = glib::user_special_dir(glib::UserDirectory::Pictures)
        .context("Could not find XDG_PICTURES_DIR")?
        // TODO Should this be translated? It is not expected that if the
        // user switches locales, videos now go to another folder.
        .join("Camera");

    std::fs::create_dir_all(&path)?;

    Ok(path)
}

static DEBUG_STR: Lazy<String> = Lazy::new(|| {
    let registry = gst::Registry::get();
    let mut version_string = String::new();

    version_string.push_str(&format!("Aperture {}\n", aperture::version()));
    version_string.push_str(&format!("{}\n", gst::version_string()));
    if let Some(pipewire_feature) = registry.lookup_feature("pipewiresrc") {
        version_string.push_str(&format!(
            "Pipewire {}\n",
            pipewire_feature
                .plugin()
                .map(|x| String::from(x.version()))
                .unwrap_or(String::from("UNKNOWN"))
        ));
    };
    version_string.push_str(&format!(
        "Gtk {}.{}.{}",
        gtk::major_version(),
        gtk::minor_version(),
        gtk::micro_version()
    ));

    version_string
});

pub fn debug_info() -> String {
    let device_provider = aperture::DeviceProvider::instance();
    let camera_info = device_provider
        .iter::<aperture::Camera>()
        .filter_map(|camera_result| {
            camera_result
                .map(|camera| format!("{}: {:#?}", camera.display_name(), camera.properties()))
                .ok()
        })
        .collect::<Vec<String>>()
        .join(",\n");

    let mut debug_string = format!("Library Details:\n\n{}\n\n", &*DEBUG_STR);

    if device_provider.camera(0).is_some() {
        debug_string.push_str("Cameras:\n\n");
        debug_string.push_str(&camera_info);
    }

    debug_string
}
