// SPDX-License-Identifier: GPL-3.0-or-later
use gettextrs::gettext;
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
