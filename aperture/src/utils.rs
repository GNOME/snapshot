pub(crate) mod caps {
    use once_cell::sync::Lazy;

    static IR_CAPS: Lazy<gst::Caps> = Lazy::new(|| {
        crate::SUPPORTED_ENCODINGS
            .iter()
            .map(|encoding| {
                gst_video::VideoCapsBuilder::for_encoding(*encoding)
                    .format(gst_video::VideoFormat::Gray8)
                    .build()
            })
            .collect()
    });

    pub(crate) fn is_infrared(cap: &gst::Caps) -> bool {
        cap.is_subset(&IR_CAPS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_infrared() {
        gst::init().expect("Failed to initalize gst");

        let infrared_caps = [
            gst_video::VideoCapsBuilder::for_encoding("image/jpeg")
                .format(gst_video::VideoFormat::Gray8)
                .build(),
            gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Gray8)
                .build(),
            [
                gst_video::VideoCapsBuilder::for_encoding("image/jpeg")
                    .format(gst_video::VideoFormat::Gray8)
                    .build(),
                gst_video::VideoCapsBuilder::for_encoding("video/x-raw")
                    .format(gst_video::VideoFormat::Gray8)
                    .build(),
            ]
            .into_iter()
            .collect(),
        ];
        let not_infrared_caps = [
            gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Gray8)
                .format(gst_video::VideoFormat::Yv12)
                .build(),
            gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Yv12)
                .build(),
            gst_video::VideoCapsBuilder::new().build(),
            [
                gst_video::VideoCapsBuilder::for_encoding("image/jpeg")
                    .format(gst_video::VideoFormat::Gray8)
                    .build(),
                gst_video::VideoCapsBuilder::for_encoding("video/x-raw").build(),
            ]
            .into_iter()
            .collect(),
        ];

        for cap in infrared_caps {
            assert!(caps::is_infrared(&cap));
        }

        for cap in not_infrared_caps {
            assert!(!caps::is_infrared(&cap));
        }
    }
}
