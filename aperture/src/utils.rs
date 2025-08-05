#[derive(Debug, PartialEq)]
pub(crate) struct Size {
    pub width: i32,
    pub height: i32,
}

pub(crate) mod caps {
    use std::sync::LazyLock;

    use super::Size;

    pub static IR_CAPS: LazyLock<gst::Caps> = LazyLock::new(|| {
        crate::SUPPORTED_ENCODINGS
            .iter()
            .map(|enc| {
                gst::Caps::builder(*enc)
                    .field("format", gst_video::VideoFormat::Gray8.to_str())
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

    pub(crate) fn best_mode(caps: &gst::Caps) -> Option<Size> {
        const MIN_WIDTH: i32 = 640;
        const MIN_HEIGHT: i32 = 480;
        const MAX_HEIGHT: i32 = 1080;
        const OPTIMAL_RATIO: f32 = 16.0 / 9.0;

        let mut best_size_optimal_ratio: Option<Size> = None;
        let mut best_size_any_ratio: Option<Size> = None;
        let mut best_size_fallback: Option<Size> = None;

        for cap in caps.iter() {
            let Ok(width) = cap.get::<i32>("width") else {
                continue;
            };
            let Ok(height) = cap.get::<i32>("height") else {
                continue;
            };

            if best_size_fallback.is_none() {
                best_size_fallback = Some(Size { width, height });
            }

            let max_width = (height as f32 * OPTIMAL_RATIO).ceil() as i32;
            if (MIN_WIDTH..=max_width).contains(&width)
                && (MIN_HEIGHT..=MAX_HEIGHT).contains(&height)
            {
                if width == (height as f32 * OPTIMAL_RATIO) as i32 {
                    if let Some(Size {
                        width: best_w,
                        height: best_h,
                    }) = best_size_optimal_ratio
                    {
                        if width >= best_w && height >= best_h {
                            best_size_optimal_ratio = Some(Size { width, height });
                        }
                    } else {
                        best_size_optimal_ratio = Some(Size { width, height });
                    }
                } else if let Some(Size {
                    width: best_w,
                    height: best_h,
                }) = best_size_any_ratio
                {
                    if width >= best_w && height >= best_h {
                        best_size_any_ratio = Some(Size { width, height });
                    }
                } else {
                    best_size_any_ratio = Some(Size { width, height });
                }
            }
        }

        best_size_optimal_ratio
            .or(best_size_any_ratio)
            .or(best_size_fallback)
    }

    pub(crate) fn best_resolution_for_fps(caps: &gst::Caps, framerate: gst::Fraction) -> gst::Caps {
        let fixed_caps = crate::SUPPORTED_ENCODINGS
            .iter()
            .map(|encoding| {
                gst::Caps::builder(*encoding)
                    .field("framerate", framerate)
                    .build()
            })
            .collect::<gst::Caps>();
        let caps_with_format = caps.intersect_with_mode(&fixed_caps, gst::CapsIntersectMode::First);

        // We try to find the biggest height smaller than `MAX_HEIGHT`p.
        if let Some(Size { height, width }) = best_mode(&caps_with_format) {
            let fixed_res = crate::SUPPORTED_ENCODINGS
                .iter()
                .map(|encoding| {
                    gst::Caps::builder(*encoding)
                        .field("width", width)
                        .field("height", height)
                        .build()
                })
                .collect::<gst::Caps>();

            caps_with_format.intersect_with_mode(&fixed_res, gst::CapsIntersectMode::First)
        } else {
            caps_with_format
        }
    }
}

// Whether the system supports h264 video encoding.
pub fn is_h264_encoding_supported() -> bool {
    let registry = gst::Registry::get();
    registry.lookup_feature("openh264enc").is_some() || registry.lookup_feature("x264enc").is_some()
}

// Whether the system supports hardware video encoding for a given format.
pub fn is_hardware_encoding_supported(format: crate::VideoFormat) -> bool {
    let registry = gst::Registry::get();
    match format {
        crate::VideoFormat::H264Mp4 => {
            registry.lookup_feature("vah264lpenc").is_some()
                || registry.lookup_feature("vah264enc").is_some()
                || registry.lookup_feature("v4l2h264enc").is_some()
        }
        crate::VideoFormat::Vp8Webm => {
            registry.lookup_feature("vavp8lpenc").is_some()
                || registry.lookup_feature("vavp8enc").is_some()
                || registry.lookup_feature("v4l2vp8enc").is_some()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_infrared() {
        gst::init().expect("Failed to initialize gst");

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
        gst::init().expect("Failed to initialize gst");

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
        gst::init().expect("Failed to initialize gst");

        let caps_1080 = [
            gst_video::VideoCapsBuilder::new()
                .width(1920)
                .height(1080)
                .build(),
            [
                gst_video::VideoCapsBuilder::new()
                    .width(1920)
                    .height(1080)
                    .build(),
                gst_video::VideoCapsBuilder::new()
                    .width(1920)
                    .height(1081)
                    .build(),
            ]
            .into_iter()
            .collect(),
            [
                gst_video::VideoCapsBuilder::new()
                    .width(1920)
                    .height(1079)
                    .build(),
                gst_video::VideoCapsBuilder::new()
                    .width(1920)
                    .height(1080)
                    .build(),
            ]
            .into_iter()
            .collect(),
            [
                gst_video::VideoCapsBuilder::new()
                    .width(2160)
                    .height(1080)
                    .build(),
                gst_video::VideoCapsBuilder::new()
                    .width(1920)
                    .height(1080)
                    .build(),
            ]
            .into_iter()
            .collect(),
        ];

        let caps_720 = [
            gst_video::VideoCapsBuilder::new()
                .width(1280)
                .height(720)
                .build(),
            [
                gst_video::VideoCapsBuilder::new()
                    .width(1280)
                    .height(720)
                    .build(),
                gst_video::VideoCapsBuilder::new()
                    .width(1280)
                    .height(1081)
                    .build(),
            ]
            .into_iter()
            .collect(),
            [
                gst_video::VideoCapsBuilder::new()
                    .width(1280)
                    .height(720)
                    .build(),
                gst_video::VideoCapsBuilder::new()
                    .width(1280)
                    .height(1080)
                    .build(),
            ]
            .into_iter()
            .collect(),
        ];

        let caps_fallback_4k = [
            gst_video::VideoCapsBuilder::new()
                .width(3840)
                .height(2160)
                .build(),
            [
                gst_video::VideoCapsBuilder::new()
                    .width(3840)
                    .height(2160)
                    .build(),
                gst_video::VideoCapsBuilder::new()
                    .width(640)
                    .height(360)
                    .build(),
            ]
            .into_iter()
            .collect(),
        ];

        let caps_fallback_small = [
            gst_video::VideoCapsBuilder::new()
                .width(640)
                .height(360)
                .build(),
            [
                gst_video::VideoCapsBuilder::new()
                    .width(640)
                    .height(360)
                    .build(),
                gst_video::VideoCapsBuilder::new()
                    .width(3840)
                    .height(2160)
                    .build(),
            ]
            .into_iter()
            .collect(),
        ];

        for cap in caps_1080 {
            assert_eq!(
                caps::best_mode(&cap),
                Some(Size {
                    width: 1920,
                    height: 1080
                })
            );
        }

        for cap in caps_720 {
            assert_eq!(
                caps::best_mode(&cap),
                Some(Size {
                    width: 1280,
                    height: 720
                })
            );
        }

        for cap in caps_fallback_4k {
            assert_eq!(
                caps::best_mode(&cap),
                Some(Size {
                    width: 3840,
                    height: 2160
                })
            );
        }

        for cap in caps_fallback_small {
            assert_eq!(
                caps::best_mode(&cap),
                Some(Size {
                    width: 640,
                    height: 360
                })
            );
        }
    }
}
