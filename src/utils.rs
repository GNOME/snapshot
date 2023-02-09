// SPDX-License-Identifier: GPL-3.0-or-later
use std::path::PathBuf;

use gtk::glib;

pub fn picture_file_name(picture_format: crate::PictureFormat) -> String {
    // Alternatively check
    // https://gitlab.gnome.org/sdroege/las-workshop-2019/-/blob/master/src/pipeline.rs.
    let format = picture_format.as_str();
    if let Ok(date_time) = glib::DateTime::now_local() {
        let year = date_time.year();
        let month = date_time.month();
        let day = date_time.day_of_month();
        let hour = date_time.hour();
        let minute = date_time.minute();
        let second = date_time.second();
        format!("IMG_{year}{month:0>2}{day:0>2}_{hour:0>2}{minute:0>2}{second:0>2}.{format}")
    } else {
        let rand = glib::random_int_range(0, 999999);
        format!("IMG_{rand}.{format}")
    }
}

pub fn video_file_name(video_format: crate::VideoFormat) -> String {
    let format = video_format.as_str();
    if let Ok(date_time) = glib::DateTime::now_local() {
        let year = date_time.year();
        let month = date_time.month();
        let day = date_time.day_of_month();
        let hour = date_time.hour();
        let minute = date_time.minute();
        let second = date_time.second();
        format!("RECORDING_{year}{month:0>2}{day:0>2}_{hour:0>2}{minute:0>2}{second:0>2}.{format}")
    } else {
        let rand = glib::random_int_range(0, 999999);
        format!("RECORDING_{rand}.{format}")
    }
}

// TODO These should return a result so we stop the file saving process
// if we fail.
pub fn videos_dir() -> PathBuf {
    let path = glib::user_special_dir(glib::UserDirectory::Videos)
        .unwrap()
        // TODO Should this be translated? It is not expected that if the
        // user switches locales, videos now go to another folder.
        .join("Snapshot");

    std::fs::create_dir_all(&path)
        .unwrap_or_else(|err| log::debug!("Could not create videos directory: {err}"));

    path
}

pub fn pictures_dir() -> PathBuf {
    let path = glib::user_special_dir(glib::UserDirectory::Pictures)
        .unwrap()
        // TODO Should this be translated? It is not expected that if the
        // user switches locales, videos now go to another folder.
        .join("Snapshot");

    std::fs::create_dir_all(&path)
        .unwrap_or_else(|err| log::debug!("Could not create pictures directory: {err}"));

    path
}
