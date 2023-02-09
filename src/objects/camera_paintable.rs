// SPDX-License-Identifier: GPL-3.0-or-later
//
// Fancy Camera with QR code detection using ZBar
//
// Pipeline:
//                            queue -- videoconvert -- zbar -- fakesink
//                         /
//     pipewiresrc -- tee  -- queue2 -- gtkpaintablesink
//                         \
//                            queue3 -- fakesink2
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use adw::prelude::*;
use glib::clone;
use gst::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene};

use crate::{config, utils};

/// Time to wait before trying to emit code-detected.
const CODE_TIMEOUT: u64 = 3;

mod imp {
    use std::cell::{Cell, RefCell};

    use once_cell::sync::{Lazy, OnceCell};

    use super::*;

    #[derive(Debug, Default)]
    pub struct CameraPaintable {
        pub pipeline: OnceCell<gst::Pipeline>,
        pub tee: OnceCell<gst::Element>,
        pub sink_paintable: OnceCell<gdk::Paintable>,
        pub pipewire_src: RefCell<Option<gst::Element>>,
        pub audio_src: RefCell<Option<gst::Element>>,
        pub sink: OnceCell<gst::Element>,
        pub recording_bin: RefCell<Option<gst::Bin>>,
        pub code_hash: Cell<Option<u64>>,

        pub flash_ani: OnceCell<adw::TimedAnimation>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CameraPaintable {
        const NAME: &'static str = "CameraPaintable";
        type Type = super::CameraPaintable;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for CameraPaintable {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.init_pipeline();
        }

        fn dispose(&self) {
            self.obj().close_pipeline();
            let bus = self.pipeline.get().unwrap().bus().unwrap();
            let _ = bus.remove_watch();
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![
                    glib::subclass::Signal::builder("code-detected")
                        .param_types([String::static_type()])
                        .build(),
                    // This is emited whenever the saving process finishes,
                    // successful or not.
                    glib::subclass::Signal::builder("picture-stored")
                        .param_types([Option::<gdk::Texture>::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }

    impl PaintableImpl for CameraPaintable {
        fn intrinsic_height(&self) -> i32 {
            if let Some(paintable) = self.sink_paintable.get() {
                paintable.intrinsic_height()
            } else {
                0
            }
        }

        fn intrinsic_width(&self) -> i32 {
            if let Some(paintable) = self.sink_paintable.get() {
                paintable.intrinsic_width()
            } else {
                0
            }
        }

        fn intrinsic_aspect_ratio(&self) -> f64 {
            if let Some(paintable) = self.sink_paintable.get() {
                paintable.intrinsic_aspect_ratio()
            } else {
                1.0
            }
        }

        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            if let Some(image) = self.sink_paintable.get() {
                image.snapshot(snapshot, width, height);

                if let Some(animation) = self.flash_ani.get() {
                    if !matches!(animation.state(), adw::AnimationState::Playing) {
                        return;
                    }
                    let alpha = easing(animation.value());

                    let rect = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
                    let black = gdk::RGBA::new(0.0, 0.0, 0.0, alpha as f32);

                    snapshot.append_color(&black, &rect);
                }
            } else {
                snapshot.append_color(
                    &gdk::RGBA::BLACK,
                    &graphene::Rect::new(0f32, 0f32, width as f32, height as f32),
                );
            }
        }
    }
}

glib::wrapper! {
    pub struct CameraPaintable(ObjectSubclass<imp::CameraPaintable>) @implements gdk::Paintable;
}

impl Default for CameraPaintable {
    fn default() -> Self {
        glib::Object::new(&[])
    }
}

impl CameraPaintable {
    pub fn set_pipewire_element(&self, element: gst::Element) {
        let imp = self.imp();

        let tee = imp.tee.get().unwrap();
        let pipeline = imp.pipeline.get().unwrap();

        pipeline.set_state(gst::State::Null).unwrap();

        if let Some(old_element) = imp.pipewire_src.replace(Some(element.clone())) {
            gst::Element::unlink_many(&[&old_element, tee]);
            pipeline.remove(&old_element).unwrap();
        }

        pipeline.add(&element).unwrap();
        gst::Element::link_many(&[&element, tee]).unwrap();
        pipeline.set_state(gst::State::Playing).unwrap();
    }

    pub fn set_pipewire_mic(&self, element: gst::Element) {
        let imp = self.imp();

        imp.audio_src.replace(Some(element.clone()));
    }

    fn init_pipeline(&self) {
        let imp = self.imp();
        let pipeline = gst::Pipeline::new(None);

        let tee = gst::ElementFactory::make("tee").build().unwrap();
        let queue = gst::ElementFactory::make("queue").build().unwrap();
        let videoconvert = gst::ElementFactory::make("videoconvert").build().unwrap();

        let zbar = gst::ElementFactory::make("zbar").build().unwrap();
        let fakesink = gst::ElementFactory::make("fakesink").build().unwrap();
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
                &tee,
                &queue,
                &videoconvert,
                &zbar,
                &fakesink,
                &queue2,
                &sink,
                &queue3,
                &fakesink2,
            ])
            .unwrap();

        gst::Element::link_many(&[&tee, &queue, &videoconvert, &zbar, &fakesink]).unwrap();

        tee.link_pads(None, &queue2, None).unwrap();

        gst::Element::link_many(&[&queue2, &sink]).unwrap();

        tee.link_pads(None, &queue3, None).unwrap();

        gst::Element::link_many(&[&queue3, &fakesink2]).unwrap();

        paintable.connect_invalidate_contents(clone!(@weak self as pt => move |_| {
            pt.invalidate_contents();
        }));

        paintable.connect_invalidate_size(clone!(@weak self as pt => move |_| {
            pt.invalidate_size();
        }));

        let bus = pipeline.bus().unwrap();
        bus.add_watch_local(
            clone!(@weak self as paintable => @default-return glib::Continue(false), move |_, msg| {
                match msg.view() {
                    gst::MessageView::Error(err) => {
                        log::error!(
                            "Error from {:?}: {} ({:?})",
                            err.src().map(|s| s.path_string()),
                            err.error(),
                            err.debug()
                        );
                    },
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

                            if success {
                                let path = s.get::<&str>("text").unwrap();
                                let file = gio::File::for_path(path);
                                let texture = gdk::Texture::from_file(&file).unwrap();
                                paintable.emit_picture_stored(Some(&texture));
                            } else {
                                paintable.emit_picture_stored(None);
                            }
                        }
                        _ => (),
                    },
                    gst::MessageView::Element(e) => {
                        if let Some(s) = e.structure() {
                            if s.name() != "barcode" {
                                return glib::Continue(true);
                            }
                            if let Ok(symbol) = s.get::<&str>("symbol") {
                                // TODO Should this be created only once?
                                let mut s = DefaultHasher::new();
                                symbol.hash(&mut s);
                                let hash: u64 = s.finish();

                                if Some(hash) != paintable.imp().code_hash.replace(Some(hash)) {
                                    paintable.emit_code_detected(symbol);

                                    let duration = std::time::Duration::from_secs(CODE_TIMEOUT);
                                    glib::timeout_add_local_once(duration, glib::clone!(@weak paintable => move || {
                                        paintable.imp().code_hash.take();
                                    }));
                                }
                            }
                        }
                    },
                    _ => (),
                }
                glib::Continue(true)
            }),
        )
        .expect("Failed to add bus watch");

        imp.sink_paintable.set(paintable).unwrap();
        imp.sink.set(fakesink2).unwrap();
        imp.tee.set(tee).unwrap();
        imp.pipeline.set(pipeline).unwrap();
    }

    pub fn close_pipeline(&self) {
        if let Some(pipeline) = self.imp().pipeline.get() {
            log::debug!("Closing pipeline");
            pipeline.set_state(gst::State::Null).unwrap();
        }
    }

    pub fn take_snapshot(&self, picture_format: crate::PictureFormat) -> anyhow::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let imp = self.imp();
        let pipeline = imp.pipeline.get().unwrap();
        let sink = imp.sink.get().unwrap();

        // Create the GStreamer caps for the output format
        let caps = match picture_format {
            crate::PictureFormat::Jpeg => gst::Caps::new_simple("image/jpeg", &[]),
            crate::PictureFormat::Png => gst::Caps::new_simple("image/png", &[]),
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
        let bus = pipeline.bus().expect("Pipeline has no bus");
        gst_video::convert_sample_async(
            &last_sample,
            &caps,
            Some(3 * gst::format::ClockTime::SECOND),
            move |res| {
                let sample = match res {
                    Err(err) => {
                        log::debug!("Failed to convert sample: {err}");
                        let _ = bus.post(create_application_warning_message(&format!(
                            "Failed to convert sample: {err}"
                        )));

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
                    let _ = bus.post(create_application_warning_message(&format!(
                        "Failed to create file {filename}"
                    )));
                    let msg = create_application_message("", false);
                    let _ = bus.post(msg);

                    return;
                };

                if let Err(err) = file.write_all(&map) {
                    log::debug!("Failed to write snapshot file {filename}: {err:?}");
                    let _ = bus.post(create_application_warning_message(&format!(
                        "Failed to write snapshot file {filename}: {err:?}"
                    )));
                    let msg = create_application_message("", false);
                    let _ = bus.post(msg);
                } else {
                    let msg = create_application_message(&format!("{}", path.display()), true);
                    let _ = bus.post(msg);
                }
            },
        );

        imp.flash_ani.get().unwrap().play();

        let settings = gio::Settings::new(config::APP_ID);
        if settings.boolean("play-shutter-sound") {
            self.play_shutter_sound();
        }

        Ok(())
    }

    // Start recording to the configured location
    pub fn start_recording(&self, format: crate::VideoFormat) -> anyhow::Result<()> {
        let imp = self.imp();
        let pipeline = imp.pipeline.get().unwrap();
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
            crate::VideoFormat::TheoraOgg => "oggmux name=mux ! queue ! filesink name=sink    queue name=video_entry ! videoconvert ! theoraenc ! queue ! mux.video_%u    audioconvert name=audio_entry ! vorbisenc ! queue ! mux.audio_%u",
        };

        let bin = gst::parse_bin_from_description(bin_description, false)?;

        let audiotestsrc = gst::ElementFactory::make("audiotestsrc").build().unwrap();

        let audio_entry = bin.by_name("audio_entry").unwrap();

        bin.add(&audiotestsrc).unwrap();
        audiotestsrc.link(&audio_entry).unwrap();

        let video_entry = bin.by_name("video_entry").unwrap();
        let video_entry_pad = video_entry.static_pad("sink").unwrap();

        let video_ghost_pad = gst::GhostPad::new(Some("video"), gst::PadDirection::Sink);
        video_ghost_pad.set_target(Some(&video_entry_pad)).unwrap();

        bin.add_pad(&video_ghost_pad).unwrap();

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
        pipeline.add(&bin).expect("Failed to add recording bin");

        // Get our tee element by name, request a new source pad from it and then link that to our
        // recording bin to actually start receiving data
        let srcpad = tee
            .request_pad_simple("src_%u")
            .expect("Failed to request new pad from tee");
        let sinkpad = bin
            .static_pad("video")
            .expect("Failed to get sink pad from recording bin");

        // If linking fails, we just undo what we did above
        if let Err(err) = srcpad.link(&sinkpad) {
            // This might fail but we don't care anymore: we're in an error path
            let _ = pipeline.remove(&bin);
            let _ = bin.set_state(gst::State::Null);

            anyhow::bail!("Failed to link recording bin: {err}");
        }

        imp.recording_bin.replace(Some(bin));

        log::debug!("Recording to {path:?}");

        Ok(())
    }

    // Stop recording if any recording was currently ongoing
    pub fn stop_recording(&self) {
        let imp = self.imp();
        // Get our recording bin, if it does not exist then nothing has to be stopped actually.
        // This shouldn't really happen
        let Some(bin) = imp.recording_bin.take() else {
            return;
        };

        // Get the source pad of the tee that is connected to the recording bin
        let sinkpad = bin
            .static_pad("video")
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
        srcpad.add_probe(gst::PadProbeType::IDLE, move |srcpad, _| {
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
            let sinkpad = sinkpad.clone();
            bin.call_async(move |_bin| {
                sinkpad.send_event(gst::event::Eos::new());
            });

            // Don't block the pad but remove the probe to let everything
            // continue as normal
            gst::PadProbeReturn::Remove
        });
    }

    fn emit_code_detected(&self, code: &str) {
        self.emit_by_name::<()>("code-detected", &[&code]);
    }

    pub fn connect_code_detected<F: Fn(&Self, &str) + 'static>(&self, f: F) {
        self.connect_local(
            "code-detected",
            false,
            glib::clone!(@weak self as obj => @default-return None, move |args: &[glib::Value]| {
                let code = args.get(1).unwrap().get::<&str>().unwrap();
                f(&obj, code);

                None
            }),
        );
    }

    fn emit_picture_stored(&self, texture: Option<&gdk::Texture>) {
        self.emit_by_name::<()>("picture-stored", &[&texture]);
    }

    pub fn connect_picture_stored<F: Fn(&Self, Option<&gdk::Texture>) + 'static>(&self, f: F) {
        self.connect_local(
            "picture-stored",
            false,
            glib::clone!(@weak self as obj => @default-return None, move |args: &[glib::Value]| {
                let texture = args.get(1).unwrap().get::<Option<gdk::Texture>>().unwrap();
                f(&obj, texture.as_ref());

                None
            }),
        );
    }

    pub fn set_picture<W: glib::IsA<gtk::Picture>>(&self, picture: &W) {
        picture.as_ref().set_paintable(Some(self));

        let target =
            adw::CallbackAnimationTarget::new(glib::clone!(@weak self as obj => move |_value| {
                obj.invalidate_contents();
            }));
        let ani = adw::TimedAnimation::new(picture.upcast_ref(), 0.0, 1.0, 250, &target);
        ani.set_easing(adw::Easing::Linear);

        self.imp().flash_ani.set(ani).unwrap();
    }

    fn play_shutter_sound(&self) {
        let uri = "resource:///org/gnome/World/Snapshot/sounds/camera-shutter.wav";
        let description = format!("playbin uri={uri}");
        let pipeline = gst::parse_launch(&description).unwrap();

        // FIXME Using the following the audio has crackling noises. But using
        // this we can remove the pulseaudio sandbox hole.
        //
        // let audio_sink = gst::ElementFactory::make("pipewiresink")
        //     .property("target-object", "44") // Find the correct path
        //     .property("client-name", crate::config::APP_ID)
        //     .build()
        //     .unwrap();
        //
        // pipeline.set_property("audio-sink", &audio_sink);

        pipeline.set_state(gst::State::Playing).unwrap();
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

fn create_application_warning_message(text: &str) -> gst::Message {
    gst::message::Application::new(
        gst::Structure::builder("warning")
            .field("text", text)
            .build(),
    )
}

#[inline]
fn easing(value: f64) -> f64 {
    let value = 1.0 - (1.0 - value).powi(3);

    value * (1.0 - value) * 4.0
}
