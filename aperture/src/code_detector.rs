// SPDX-License-Identifier: GPL-3.0-or-later
use std::sync::LazyLock;

use gst_video::{prelude::*, video_frame::VideoFrameRef};
use gtk::glib;

pub static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "code-detector",
        gst::DebugColorFlags::empty(),
        Some("QR code detector"),
    )
});

const DETECTOR_CAPS: &[gst_video::VideoFormat] = &[
    gst_video::VideoFormat::Gray8,
    gst_video::VideoFormat::I420,
    gst_video::VideoFormat::Yv12,
    gst_video::VideoFormat::Nv12,
    gst_video::VideoFormat::Nv21,
    gst_video::VideoFormat::Y41b,
    gst_video::VideoFormat::Y42b,
    gst_video::VideoFormat::Yuv9,
    gst_video::VideoFormat::Yvu9,
];

mod imp {
    use std::sync::Mutex;

    use gst::{subclass::prelude::*, BufferRef};
    use gst_video::subclass::prelude::*;

    use super::*;

    #[derive(Default)]
    pub struct QrCodeDetector {
        pub last_code: Mutex<Option<Vec<u8>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for QrCodeDetector {
        const NAME: &'static str = "QrCodeDetector";
        type Type = super::QrCodeDetector;
        type ParentType = gst_video::VideoFilter;
    }

    impl ObjectImpl for QrCodeDetector {}
    impl GstObjectImpl for QrCodeDetector {}

    impl ElementImpl for QrCodeDetector {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> =
                LazyLock::new(|| {
                    gst::subclass::ElementMetadata::new(
                        "QR Code detector Sink",
                        "Sink/Video/QrCode",
                        "A QR code detector",
                        "Kévin Commaille <zecakeh@tedomum.fr>",
                    )
                });

            Some(&*ELEMENT_METADATA)
        }

        fn pad_templates() -> &'static [gst::PadTemplate] {
            static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
                let caps = gst_video::video_make_raw_caps(DETECTOR_CAPS)
                    .any_features()
                    .build();

                vec![
                    gst::PadTemplate::new(
                        "src",
                        gst::PadDirection::Src,
                        gst::PadPresence::Always,
                        &caps,
                    )
                    .unwrap(),
                    gst::PadTemplate::new(
                        "sink",
                        gst::PadDirection::Sink,
                        gst::PadPresence::Always,
                        &caps,
                    )
                    .unwrap(),
                ]
            });

            PAD_TEMPLATES.as_ref()
        }
    }

    impl BaseTransformImpl for QrCodeDetector {
        const MODE: gst_base::subclass::BaseTransformMode =
            gst_base::subclass::BaseTransformMode::AlwaysInPlace;
        const PASSTHROUGH_ON_SAME_CAPS: bool = false;
        const TRANSFORM_IP_ON_PASSTHROUGH: bool = true;
    }

    impl VideoFilterImpl for QrCodeDetector {
        fn transform_frame_ip(
            &self,
            frame: &mut VideoFrameRef<&mut BufferRef>,
        ) -> Result<gst::FlowSuccess, gst::FlowError> {
            let now = std::time::Instant::now();

            // all formats we support start with an 8-bit Y plane. We don't need
            // to know about the chroma plane(s)
            let data = frame.comp_data(0).unwrap();
            let width = frame.comp_stride(0) as usize;
            let height = frame.height() as usize;

            let mut image = rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| {
                data[x + (y * width)]
            });
            let grids = image.detect_grids();

            for grid in grids {
                let mut decoded = Vec::new();

                match grid.decode_to(&mut decoded) {
                    Ok(_) => {
                        let mut previous_code = self.last_code.lock().unwrap();
                        if previous_code.as_ref() != Some(&decoded) {
                            previous_code.replace(decoded.clone());

                            let bytes = glib::Bytes::from_owned(decoded);
                            let structure = gst::Structure::builder("qrcode")
                                .field("payload", bytes)
                                .build();
                            let msg = gst::message::Element::new(structure);
                            self.post_message(msg);
                        }
                    }
                    Err(e) => {
                        gst::warning!(CAT, "Failed to decode QR code: {e}");
                    }
                }
            }

            gst::trace!(
                CAT,
                "Spent {}ms to detect qr code",
                now.elapsed().as_millis()
            );

            Ok(gst::FlowSuccess::Ok)
        }
    }
}

glib::wrapper! {
    pub struct QrCodeDetector(ObjectSubclass<imp::QrCodeDetector>)
        @extends gst::Object, gst::Element, gst_base::BaseTransform, gst_video::VideoFilter;
}

impl QrCodeDetector {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for QrCodeDetector {
    fn default() -> Self {
        glib::Object::new()
    }
}

unsafe impl Send for QrCodeDetector {}
unsafe impl Sync for QrCodeDetector {}
