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

    /// Limits FPS to `crate::MAXIMUM_RATE`.
    pub(crate) fn limit_fps(caps: &gst::Caps) -> gst::Caps {
        caps.intersect_with_mode(&crate::SUPPORTED_CAPS, gst::CapsIntersectMode::First)
    }

    pub fn best_height(caps: &gst::Caps, for_height: i32) -> Option<i32> {
        let heights: Vec<i32> = caps
            .iter()
            .filter_map(|s| s.get::<i32>("height").ok())
            .collect();

        heights
            .iter()
            .filter(|h| for_height >= **h)
            .max()
            .copied()
            .or_else(|| {
                // If not, we pick the smallest height bigger than `MAX_HEIGHT`.
                heights.into_iter().filter(|h| for_height <= *h).min()
            })
    }

    pub(crate) fn best_resolution_for_fps(caps: &gst::Caps, framerate: gst::Fraction) -> gst::Caps {
        // There are multiple aspect rations for 1080p. Therefore, we look for
        // height rather than width.
        const MAX_HEIGHT: i32 = 1080;

        let fixed_caps = crate::SUPPORTED_ENCODINGS
            .iter()
            .map(|encoding| {
                gst_video::VideoCapsBuilder::for_encoding(*encoding)
                    .framerate(framerate)
                    .build()
            })
            .collect::<gst::Caps>();
        let caps_with_format = caps.intersect_with_mode(&fixed_caps, gst::CapsIntersectMode::First);

        // We try to find the bigest height smaller than `MAX_HEIGHT`p.
        let best_height: Option<i32> = best_height(&caps_with_format, MAX_HEIGHT);

        if let Some(height) = best_height {
            let fixed_res = crate::SUPPORTED_ENCODINGS
                .iter()
                .map(|encoding| {
                    gst_video::VideoCapsBuilder::for_encoding(*encoding)
                        .height(height)
                        .build()
                })
                .collect::<gst::Caps>();

            caps_with_format.intersect_with_mode(&fixed_res, gst::CapsIntersectMode::First)
        } else {
            caps_with_format
        }
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

    #[test]
    fn test_limit_fps() {
        gst::init().expect("Failed to initalize gst");

        let good_caps = [
            gst_video::VideoCapsBuilder::new()
                .framerate(gst::Fraction::new(0, 1))
                .build(),
            gst_video::VideoCapsBuilder::new()
                .framerate(gst::Fraction::new(crate::MAXIMUM_RATE, 1))
                .build(),
            gst_video::VideoCapsBuilder::new()
                .framerate_range(
                    gst::Fraction::new(0, 1)..=gst::Fraction::new(crate::MAXIMUM_RATE, 1),
                )
                .build(),
            gst_video::VideoCapsBuilder::new().build(),
        ];

        let bad_caps = [gst_video::VideoCapsBuilder::new()
            .framerate(gst::Fraction::new(crate::MAXIMUM_RATE + 1, 1))
            .build()];

        for cap in good_caps {
            assert!(!caps::limit_fps(&cap).is_empty());
        }

        for cap in bad_caps {
            assert!(caps::limit_fps(&cap).is_empty());
        }
    }

    #[test]
    fn test_best_height() {
        gst::init().expect("Failed to initalize gst");

        let caps_1080 = [
            gst_video::VideoCapsBuilder::new().height(1080).build(),
            [
                gst_video::VideoCapsBuilder::new().height(1080).build(),
                gst_video::VideoCapsBuilder::new().height(1081).build(),
            ]
            .into_iter()
            .collect(),
            [
                gst_video::VideoCapsBuilder::new().height(1079).build(),
                gst_video::VideoCapsBuilder::new().height(1080).build(),
            ]
            .into_iter()
            .collect(),
        ];

        let caps_720 = [
            gst_video::VideoCapsBuilder::new().height(720).build(),
            [
                gst_video::VideoCapsBuilder::new().height(720).build(),
                gst_video::VideoCapsBuilder::new().height(1081).build(),
            ]
            .into_iter()
            .collect(),
        ];

        for cap in caps_1080 {
            assert_eq!(caps::best_height(&cap, 1080), Some(1080));
        }

        for cap in caps_720 {
            assert_eq!(caps::best_height(&cap, 1080), Some(720));
        }
    }
}
