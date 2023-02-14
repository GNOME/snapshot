// SPDX-License-Identifier: GPL-3.0-or-later
//
// Fancy Camera with QR code detection using ZBar
//
// Pipeline:
//                                 queue -- videoconvert -- zbar -- fakesink
//                              /
//     pipewiresrc -- videoflip -- tee  -- queue2 -- gtkpaintablesink
//                              \
//                                 queue3 -- fakesink2
//
use std::path::PathBuf;

use glib::clone;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gtk::{gdk, glib};

use crate::utils;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    CodeDetected(String),
    PictureSaved(Option<PathBuf>),
}

mod imp {
    use super::*;

    use std::sync::{Arc, Mutex};

    use once_cell::sync::OnceCell;

    #[derive(Debug, Default)]
    pub struct Pipeline {
        pub tee: OnceCell<gst::Element>,
        pub paintablesink: OnceCell<gst::Element>,
        pub start: OnceCell<gst::Element>,
        pub pipewire_src: Arc<Mutex<Option<gst::Element>>>,
        pub sink: OnceCell<gst::Element>,
        pub recording_bin: Arc<Mutex<Option<gst::Bin>>>,

        pub sender: OnceCell<glib::Sender<Action>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Pipeline {
        const NAME: &'static str = "Pipeline";
        type Type = super::Pipeline;
        type ParentType = gst::Pipeline;
    }

    impl ObjectImpl for Pipeline {
        fn constructed(&self) {
            self.parent_constructed();

            let pipeline = self.obj();
            pipeline.set_message_forward(true);

            let videoflip = gst::ElementFactory::make("videoflip")
                .property_from_str("video-direction", "auto")
                .build()
                .unwrap();
            let tee = gst::ElementFactory::make("tee").build().unwrap();

            let queue = gst::ElementFactory::make("queue").build().unwrap();
            let videoconvert = gst::ElementFactory::make("videoconvert").build().unwrap();
            let zbar = gst::ElementFactory::make("zbar").build().unwrap();
            let fakesink = gst::ElementFactory::make("fakesink").build().unwrap();

            let zbarbin = gst::Bin::default();
            zbarbin
                .add_many(&[&queue, &videoconvert, &zbar, &fakesink])
                .unwrap();
            gst::Element::link_many(&[&queue, &videoconvert, &zbar, &fakesink]).unwrap();
            zbarbin
                .add_pad(
                    &gst::GhostPad::with_target(Some("sink"), &queue.static_pad("sink").unwrap())
                        .unwrap(),
                )
                .unwrap();

            let queue2 = gst::ElementFactory::make("queue").build().unwrap();

            let queue3 = gst::ElementFactory::make("queue").build().unwrap();
            let fakesink2 = gst::ElementFactory::make("fakesink").build().unwrap();

            let paintablesink = gst::ElementFactory::make("gtk4paintablesink")
                .build()
                .unwrap();
            let paintable = paintablesink.property::<gdk::Paintable>("paintable");

            // create the appropriate sink depending on the environment we are running
            // Check if the paintablesink initialized a gl-context, and if so put it
            // behind a glsinkbin so we keep the buffers on the gpu.
            let sink = if paintable
                .property::<Option<gdk::GLContext>>("gl-context")
                .is_some()
            {
                gst::ElementFactory::make("glsinkbin")
                    .property("sink", &paintablesink)
                    .build()
                    .unwrap()
            } else {
                let bin = gst::Bin::default();
                let convert = gst::ElementFactory::make("videoconvert").build().unwrap();

                bin.add(&convert).unwrap();
                bin.add(&paintablesink).unwrap();
                convert.link(&paintablesink).unwrap();

                bin.add_pad(
                    &gst::GhostPad::with_target(Some("sink"), &convert.static_pad("sink").unwrap())
                        .unwrap(),
                )
                .unwrap();

                bin.upcast()
            };

            pipeline
                .add_many(&[
                    &videoflip,
                    &tee,
                    zbarbin.upcast_ref(),
                    &queue2,
                    &sink,
                    &queue3,
                    &fakesink2,
                ])
                .unwrap();

            videoflip.link(&tee).unwrap();

            tee.link_pads(None, &zbarbin, None).unwrap();

            tee.link_pads(None, &queue2, None).unwrap();

            gst::Element::link_many(&[&queue2, &sink]).unwrap();

            tee.link_pads(None, &queue3, None).unwrap();

            gst::Element::link_many(&[&queue3, &fakesink2]).unwrap();

            let bus = pipeline.bus().unwrap();
            bus.add_watch_local(
                clone!(@weak pipeline => @default-return glib::Continue(false), move |_, msg| {
                    match msg.view() {
                        gst::MessageView::Error(err) => {
                            log::error!(
                                "Error from {:?}: {} ({:?})",
                                err.src().map(|s| s.path_string()),
                                err.error(),
                                err.debug()
                            );
                        },
                        gst::MessageView::Warning(err) => {
                            log::warn!(
                                "Warning from {:?}: {} ({:?})",
                                err.src().map(|s| s.path_string()),
                                err.error(),
                                err.debug()
                            );
                        }
                        gst::MessageView::Application(msg) => match msg.structure() {
                            // Here we can send ourselves messages from any thread and show them to the user in
                            // the UI in case something goes wrong
                            Some(s) if s.name() == "warning" => {
                                let text = s.get::<&str>("text").expect("Warning message without text");
                                log::error!("{text}");
                            }
                            Some(s) if s.name() == "picture-saved" => {
                                // TODO Is it possible to sent a Path directly here?
                                let success = s.get::<bool>("success").unwrap();

                                let sender = pipeline.imp().sender.get().unwrap();
                                if success {
                                    let path = s.get::<&str>("text").unwrap();
                                    let _ = sender.send(Action::PictureSaved(Some(path.into())));
                                } else {
                                    let _ = sender.send(Action::PictureSaved(None));
                                }
                            }
                            _ => (),
                        },
                        gst::MessageView::Element(e) => {
                            match e.structure() {
                                Some(s) if s.name() == "barcode" => {
                                    if let Ok(symbol) = s.get::<String>("symbol") {
                                        // TODO Should this be created only once?

                                        let sender = pipeline.imp().sender.get().unwrap();
                                        let _ = sender.send(Action::CodeDetected(symbol));
                                    }
                                }
                                Some(s) if s.name() == "GstBinForwarded" => {
                                    let msg = s
                                        .get::<gst::Message>("message")
                                        .expect("Failed to get forwarded message");

                                    if let gst::MessageView::Eos(..) = msg.view() {
                                        log::debug!("Got EOS");
                                        let Some(src) = msg.src().take() else {
                                            return glib::Continue(true);
                                        };
                                        let bin = src.downcast_ref::<gst::Element>().unwrap();

                                        // And then asynchronously remove it and set its state to Null
                                        pipeline.call_async(glib::clone!(@weak bin => move |pipeline| {
                                            // Ignore if the bin was not in the pipeline anymore for whatever
                                            // reason. It's not a problem
                                            let _ = pipeline.remove(&bin);

                                            if let Err(err) = bin.set_state(gst::State::Null) {
                                                log::error!("Failed to stop recording: {err}");
                                            }
                                        }));
                                    }
                                },
                                _ => (),

                            }},
                        _ => (),
                    }
                    glib::Continue(true)
                }))
               .expect("Failed to add bus watch");

            self.start.set(videoflip).unwrap();
            self.paintablesink.set(paintablesink).unwrap();
            self.sink.set(fakesink2).unwrap();
            self.tee.set(tee).unwrap();
        }

        fn dispose(&self) {
            self.obj().close();
            let bus = self.obj().bus().unwrap();
            let _ = bus.remove_watch();
        }
    }

    impl GstObjectImpl for Pipeline {}
    impl ElementImpl for Pipeline {}
    impl BinImpl for Pipeline {}
    impl PipelineImpl for Pipeline {}
}

glib::wrapper! {
    pub struct Pipeline(ObjectSubclass<imp::Pipeline>)
        @extends gst::Object, gst::Element, gst::Bin, gst::Pipeline;
}

impl Pipeline {
    pub fn new(sender: glib::Sender<Action>) -> Self {
        let pipeline: Self = glib::Object::new();
        pipeline.imp().sender.set(sender).unwrap();

        pipeline
    }

    pub fn paintable(&self) -> gdk::Paintable {
        self.imp()
            .paintablesink
            .get()
            .unwrap()
            .property("paintable")
    }

    pub fn close(&self) {
        log::debug!("Closing pipeline");
        self.set_state(gst::State::Null).unwrap();
    }

    pub fn take_snapshot(&self, picture_format: crate::PictureFormat) -> anyhow::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let imp = self.imp();
        let sink = imp.sink.get().unwrap();

        // Create the GStreamer caps for the output format
        let caps = match picture_format {
            crate::PictureFormat::Jpeg => gst::Caps::builder("image/jpeg").build(),
            crate::PictureFormat::Png => gst::Caps::builder("image/png").build(),
        };

        let Some(last_sample) = sink.property::<Option<gst::Sample>>("last-sample") else {
            // We have no sample to store yet
            return Ok(());
        };

        // Create the filename and open the file writable
        let filename = utils::picture_file_name(picture_format);
        let path = utils::pictures_dir().join(&filename);

        // Then convert it from whatever format we got to PNG or JPEG as requested and write it out
        log::debug!("Writing snapshot to {}", path.display());
        let bus = self.bus().expect("Pipeline has no bus");
        gst_video::convert_sample_async(
            &last_sample,
            &caps,
            Some(3 * gst::format::ClockTime::SECOND),
            move |res| {
                let sample = match res {
                    Err(err) => {
                        log::debug!("Failed to convert sample: {err}");

                        // We need to say that the picture saving process
                        // finished in all branches of the closure.
                        let msg = create_application_message("", false);
                        let _ = bus.post(msg);

                        return;
                    }
                    Ok(sample) => sample,
                };

                let buffer = sample.buffer().expect("Failed to get buffer");
                let map = buffer
                    .map_readable()
                    .expect("Failed to map buffer readable");

                let Ok(mut file) = File::create(&path) else {
                    log::debug!("Failed to create file {filename}");
                    let msg = create_application_message("", false);
                    let _ = bus.post(msg);

                    return;
                };

                if let Err(err) = file.write_all(&map) {
                    log::debug!("Failed to write snapshot file {filename}: {err:?}");
                    let msg = create_application_message("", false);
                    let _ = bus.post(msg);
                } else {
                    let msg = create_application_message(&format!("{}", path.display()), true);
                    let _ = bus.post(msg);
                }
            },
        );

        Ok(())
    }

    // Start recording to the configured location
    pub fn start_recording(&self, format: crate::VideoFormat) -> anyhow::Result<()> {
        let imp = self.imp();
        let tee = imp.tee.get().unwrap();

        let bin_description = match format {
            // FIXME Does not work. passing
            // `gst-launch-1.0 -e pipewiresrc path=42 ! videoconvert ! x264enc tune=zerolatency ! video/x-h264,profile=baseline ! mp4mux ! filesink location=test.mp4`
            // does work.
            crate::VideoFormat::H264Mp4 => "queue ! videoconvert ! x264enc tune=zerolatency ! video/x-h264,profile=baseline ! mp4mux ! queue! filesink name=sink",
            // FIXME H265 is not working. mp4mux does not support it.
            crate::VideoFormat::H265Mp4 => "queue ! videoconvert ! x265enc tune=zerolatency ! video/x-h265,profile=baseline ! mp4mux ! queue ! filesink name=sink",
            // FIXME Quality using webm is super low. Does not work with Totem.
            crate::VideoFormat::Vp8Webm => "queue ! videoconvert ! vp8enc deadline=1 ! webmmux ! queue ! filesink name=sink",
            // TODO For audio, the following works on the cli:
            // gst-launch-1.0 -e pipewiresrc path=42 ! videoconvert ! theoraenc ! oggmux name=mux ! queue ! filesink location=test.ogg    pipewiresrc path=45 ! audioconvert ! vorbisenc ! mux.
            // crate::VideoFormat::TheoraOgg => "oggmux name=mux ! queue ! filesink name=sink    queue name=video_entry ! videoconvert ! theoraenc ! queue ! mux.video_%u    audioconvert name=audio_entry ! vorbisenc ! queue ! mux.audio_%u",
            crate::VideoFormat::TheoraOgg => "videoconvert ! queue ! theoraenc ! queue ! oggmux ! filesink name=sink",
        };

        let bin = gst::parse_bin_from_description(bin_description, true)?;

        // Get our file sink element by its name and set the location where to write the recording
        let sink = bin
            .by_name("sink")
            .expect("Recording bin has no sink element");

        let filename = utils::video_file_name(format);
        let path = utils::videos_dir().join(filename);

        // All strings in GStreamer are UTF8, we need to convert the path to UTF8 which in theory
        // can fail
        sink.set_property("location", &(path.to_str().unwrap()));

        // First try setting the recording bin to playing: if this fails we know this before it
        // potentially interferred with the other part of the pipeline
        bin.set_state(gst::State::Playing)?;

        // Add the bin to the pipeline. This would only fail if there was already a bin with the
        // same name, which we ensured can't happen
        self.add(&bin).expect("Failed to add recording bin");

        // Get our tee element by name, request a new source pad from it and then link that to our
        // recording bin to actually start receiving data
        let srcpad = tee
            .request_pad_simple("src_%u")
            .expect("Failed to request new pad from tee");
        let sinkpad = bin
            .static_pad("sink")
            .expect("Failed to get sink pad from recording bin");

        // If linking fails, we just undo what we did above
        if let Err(err) = srcpad.link(&sinkpad) {
            // This might fail but we don't care anymore: we're in an error path
            let _ = self.remove(&bin);
            let _ = bin.set_state(gst::State::Null);

            anyhow::bail!("Failed to link recording bin: {err}");
        }

        *imp.recording_bin.lock().unwrap() = Some(bin);

        log::debug!("Recording to {path:?}");

        Ok(())
    }

    // Stop recording if any recording was currently ongoing
    pub fn stop_recording(&self) {
        let imp = self.imp();
        // Get our recording bin, if it does not exist then nothing has to be stopped actually.
        // This shouldn't really happen
        //
        let Some(bin) = imp.recording_bin.lock().unwrap().take() else {
            return;
        };

        // Get the source pad of the tee that is connected to the recording bin
        let sinkpad = bin
            .static_pad("sink")
            .expect("Failed to get sink pad from recording bin");
        let Some(srcpad) = sinkpad.peer() else {
            return;
        };

        log::debug!("Stopping recording");

        // Once the tee source pad is idle and we wouldn't interfere with any data flow, unlink the
        // tee and the recording bin and finalize the recording bin by sending it an end-of-stream
        // event
        //
        // Once the end-of-stream event is handled by the whole recording bin, we get an
        // end-of-stream message from it in the message handler and the shut down the recording bin
        // and remove it from the pipeline
        //
        // The closure below might be called directly from the main UI thread here or at a later
        // time from a GStreamer streaming thread
        srcpad.add_probe(gst::PadProbeType::IDLE, glib::clone!(@weak sinkpad, @weak bin => @default-return gst::PadProbeReturn::Remove, move |srcpad, _| {
            // Get the parent of the tee source pad, i.e. the tee itself
            let tee = srcpad
                .parent()
                .and_then(|parent| parent.downcast::<gst::Element>().ok())
                .expect("Failed to get tee source pad parent");

            // Unlink the tee source pad and then release it
            //
            // If unlinking fails we don't care, just make sure that the
            // pad is actually released
            let _ = srcpad.unlink(&sinkpad);
            tee.release_request_pad(srcpad);

            // Asynchronously send the end-of-stream event to the sinkpad as this might block for a
            // while and our closure here might've been called from the main UI thread
            bin.call_async(glib::clone!(@weak sinkpad => move |_bin| {
                sinkpad.send_event(gst::event::Eos::new());
            }));

            // Don't block the pad but remove the probe to let everything
            // continue as normal
            gst::PadProbeReturn::Remove
        }));
    }

    // FIXME This is probably wrong.
    pub fn set_pipewire_element(&self, element: gst::Element) {
        let imp = self.imp();

        let start = imp.start.get().unwrap();

        let mut guard = imp.pipewire_src.lock().unwrap();
        if let Some(old_element) = guard.take() {
            self.set_state(gst::State::Null).unwrap();
            old_element.unlink(start);
            self.remove(&old_element).unwrap();
        }
        self.add(&element).unwrap();

        element.link(start).unwrap();
        self.set_state(gst::State::Playing).unwrap();

        *guard = Some(element);
    }
}

fn create_application_message(text: &str, success: bool) -> gst::Message {
    gst::message::Application::new(
        gst::Structure::builder("picture-saved")
            .field("text", text)
            .field("success", success)
            .build(),
    )
}
