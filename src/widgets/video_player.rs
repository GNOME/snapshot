// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use super::*;

    #[derive(Debug, Default)]
    pub struct VideoPlayer {
        pub media_file: gtk::MediaFile,
        pub picture: gtk::Picture,
        pub controls: gtk::MediaControls,
        pub thumbnail: OnceCell<gdk::Texture>,
        pub has_thumbnail: Cell<bool>,

        pub signal_handler: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VideoPlayer {
        const NAME: &'static str = "VideoPlayer";
        type Type = super::VideoPlayer;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for VideoPlayer {
        fn constructed(&self) {
            self.parent_constructed();

            let widget = self.obj();

            self.picture.set_paintable(Some(&self.media_file));
            self.controls.set_media_stream(Some(&self.media_file));
            self.controls.set_valign(gtk::Align::Center);
            self.controls.set_halign(gtk::Align::Fill);
            self.controls.set_hexpand(true);
            self.controls.add_css_class("videoplayercontrols");

            widget.set_child(Some(&self.picture));

            let id = self.media_file.connect_invalidate_contents(
                glib::clone!(@weak widget => move |media_file| {
                    widget.imp().has_thumbnail.set(true);
                    if let Some(id) = widget.imp().signal_handler.take() {
                        media_file.disconnect(id);
                    }
                }),
            );
            self.signal_handler.replace(Some(id));
        }
    }
    impl WidgetImpl for VideoPlayer {}
    impl BinImpl for VideoPlayer {}
}

glib::wrapper! {
    pub struct VideoPlayer(ObjectSubclass<imp::VideoPlayer>)
        @extends gtk::Widget, adw::Bin;
}

impl Default for VideoPlayer {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl VideoPlayer {
    // Generates a thumbnail.
    pub async fn thumbnail(&self) -> Option<&gdk::Texture> {
        let imp = self.imp();
        if imp.thumbnail.get().is_none() {
            // We have to wait till the stream is prepared before trying to
            // snapshot it.
            let texture = if self.imp().has_thumbnail.get() {
                self.snapshot_thumbnail()?
            } else {
                let (sender, receiver) =
                    futures_channel::oneshot::channel::<Option<gdk::Texture>>();
                let sender = std::sync::Arc::new(std::sync::Mutex::new(Some(sender)));

                let id = self.stream().connect_invalidate_contents(
                    glib::clone!(@weak self as obj, @strong sender => move |_| {
                        let opt_texture = obj.snapshot_thumbnail();

                        let mut guard = sender.lock().unwrap();
                        if let Some(sender) = guard.take() {
                            let _ = sender.send(opt_texture);
                        };
                    }),
                );

                let texture = receiver.await.unwrap();
                self.stream().disconnect(id);

                texture?
            };

            self.imp().thumbnail.set(texture).unwrap();
        }

        self.imp().thumbnail.get()
    }

    pub fn pause(&self) {
        self.imp().media_file.pause();
    }

    fn snapshot_thumbnail(&self) -> Option<gdk::Texture> {
        let imp = self.imp();

        let snapshot = gtk::Snapshot::new();
        // Any value bigger than the size of the thumbnail button in camera.ui
        // is ok.
        const SIZE: f64 = 88.0;

        let t_width = imp.media_file.intrinsic_width() as f64;
        let t_height = imp.media_file.intrinsic_height() as f64;
        let t_ratio = t_width / t_height;

        let width = if t_ratio > 1.0 { SIZE * t_ratio } else { SIZE };
        let height = if t_ratio > 1.0 { SIZE } else { SIZE / t_ratio };

        self.imp().media_file.snapshot(&snapshot, width, height);
        let node = snapshot.to_node()?;

        let renderer = self.native()?.renderer()?;
        let texture = renderer.render_texture(&node, None);

        Some(texture)
    }

    pub fn set_file(&self, file: &gio::File) {
        let imp = self.imp();

        imp.media_file.set_file(Some(file));

        if let Some(basename) = file.basename() {
            let label = basename.display().to_string();
            imp.picture
                .update_property(&[gtk::accessible::Property::Label(&label)]);
        }
    }

    pub fn stream(&self) -> &gtk::MediaStream {
        self.imp().media_file.upcast_ref()
    }
}
