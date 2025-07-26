// SPDX-License-Identifier: GPL-3.0-or-later
use std::path::PathBuf;
use std::sync::LazyLock;

use anyhow::Context;
use gettextrs::gettext;
use gst::prelude::*;
use gtk::prelude::*;
use gtk::{gio, glib};

use crate::i18n::i18n_f;

const DATE_FORMAT: &str = "%Y-%m-%d %H-%M-%S.%f";

pub fn picture_file_name(picture_format: crate::PictureFormat) -> String {
    // Alternatively check
    // https://gitlab.gnome.org/sdroege/las-workshop-2019/-/blob/master/src/pipeline.rs.
    let format = picture_format.as_str();
    let file_name = if let Ok(date_time) = glib::DateTime::now_local() {
        let f_date = date_time.format(DATE_FORMAT).unwrap();
        // TRANSLATORS Do NOT translate {date}. This will appear as, e.g. "Photo
        // from 2023-05-21 11-05-59.12345" and it will be used as a file name.
        i18n_f("Photo from {date}", &[("date", &f_date)])
    } else {
        let rand = glib::random_int_range(0, 999999).to_string();
        // TRANSLATORS Do NOT translate {number}. This will appear as, e.g.
        // "Photo 12345" and it will be used as a file name.
        i18n_f("Photo {number}", &[("number", &rand)])
    };

    format!("{file_name}.{format}")
}

pub fn video_file_name(video_format: aperture::VideoFormat) -> String {
    let format = video_format.extension_as_str();
    let file_name = if let Ok(date_time) = glib::DateTime::now_local() {
        let f_date = date_time.format(DATE_FORMAT).unwrap();
        // TRANSLATORS Do NOT translate {date}. This will appear as, e.g.
        // "Recording from 2023-05-21 11-05-59.12345" and it will be used as a
        // file name.
        i18n_f("Recording from {date}", &[("date", &f_date)])
    } else {
        let rand = glib::random_int_range(0, 999999).to_string();
        // TRANSLATORS Do NOT translate {number}. This will appear as, e.g.
        // "Recording 12345" and it will be used as a file name.
        i18n_f("Recording {number}", &[("number", &rand)])
    };

    format!("{file_name}.{format}")
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

static DEBUG_STR: LazyLock<String> = LazyLock::new(|| {
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
                .map(|camera| {
                    let d = camera.property::<gst::Device>("device");
                    format!(
                        "{} {}: {:#?}",
                        camera.display_name(),
                        d.device_class(),
                        camera.properties()
                    )
                })
                .ok()
        })
        .collect::<Vec<String>>()
        .join(",\n");

    let mut debug_string = format!("Library Details:\n\n{}", &*DEBUG_STR);

    if device_provider.camera(0).is_some() {
        debug_string.push_str("\n\nCameras:\n\n");
        debug_string.push_str(&camera_info);
    }

    debug_string
}

pub fn gallery_item_menu(is_picture: bool) -> gio::Menu {
    let menu = gio::Menu::new();
    if is_picture {
        menu.append(Some(&gettext("_Copy Picture")), Some("gallery.copy"));
    } else {
        menu.append(Some(&gettext("_Copy Video")), Some("gallery.copy"));
    }
    menu.append(Some(&gettext("_Delete")), Some("gallery.delete"));
    menu.freeze();

    menu
}
