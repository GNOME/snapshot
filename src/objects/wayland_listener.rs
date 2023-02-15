// SPDX-License-Identifier: GPL-3.0-or-later
use std::os::fd::AsRawFd;

use futures_util::future::poll_fn;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};
use wayland_client::protocol::wl_display::WlDisplay;
use wayland_client::EventQueue;
use wayland_client::WEnum;
use wayland_client::{
    protocol::{wl_output, wl_registry},
    Proxy, QueueHandle,
};

const WL_OUTPUT_V1: u32 = 1;

#[derive(Default, Debug)]
struct State {
    transform: Option<crate::Transform>,
    supports_output: bool,
}

mod imp {
    use super::*;

    use std::cell::{Cell, RefCell};

    use glib::Properties;
    use once_cell::sync::OnceCell;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::WaylandListener)]
    pub struct WaylandListener {
        #[property(get, set, construct_only)]
        display: OnceCell<gdk::Display>,
        #[property(get, set = Self::set_transform, explicit_notify, builder(Default::default()))]
        transform: Cell<crate::Transform>,

        pub supports_output: Cell<bool>,
        pub fd_watch: RefCell<Option<glib::source::SourceId>>,
    }

    impl WaylandListener {
        fn set_transform(&self, transform: crate::Transform) {
            if transform != self.transform.replace(transform) {
                self.obj().notify("transform");
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WaylandListener {
        const NAME: &'static str = "WaylandListener";
        type Type = super::WaylandListener;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for WaylandListener {
        fn constructed(&self) {
            self.parent_constructed();

            if let Some(display) = self
                .display
                .get()
                .unwrap()
                .downcast_ref::<gdk4wayland::WaylandDisplay>()
            {
                if let Some(wl_display) = display.wl_display() {
                    if let Err(err) = self.obj().start_listening(&wl_display) {
                        log::error!("Error listening to wayland: {err}");
                    };
                };
            }
        }

        fn dispose(&self) {
            if let Some(source) = self.fd_watch.take() {
                source.remove();
            }
        }

        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            Self::derived_set_property(self, id, value, pspec)
        }
    }
}

glib::wrapper! {
    pub struct WaylandListener(ObjectSubclass<imp::WaylandListener>);
}

impl WaylandListener {
    pub fn new(display: gdk::Display) -> Self {
        glib::Object::builder().property("display", display).build()
    }

    fn start_listening(&self, display: &WlDisplay) -> anyhow::Result<()> {
        let imp = self.imp();

        let Some(backend) = display.backend().upgrade() else {
            log::warn!("Wayland display didn't have a backend");
            return Ok(());
        };
        let conn = wayland_client::Connection::from_backend(backend);

        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();

        let mut state = State::default();
        let _registry = display.get_registry(&qhandle, ());
        // TODO This should be done similar to how we do `run()` so that it does
        // not block.
        event_queue.roundtrip(&mut state)?;

        imp.supports_output.replace(state.supports_output);

        if !imp.supports_output.get() {
            log::warn!("Wayland compositor does not support the wl_output protocol");
            return Ok(());
        }

        // FIXME Prepare to read from the socket, this might stop working in the
        // future see
        //
        // https://github.com/Smithay/wayland-rs/pull/57
        // https://github.com/Smithay/wayland-rs/issues/570
        let fd = conn.prepare_read()?.connection_fd().as_raw_fd();
        let source = glib::source::unix_fd_add_local(fd, glib::IOCondition::IN, move |_, _| {
            glib::Continue(prepare_read(&conn).is_ok())
        });
        imp.fd_watch.replace(Some(source));

        let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        let ctx = glib::MainContext::default();
        ctx.spawn_local(glib::clone!(@weak self as obj => async move {
            if let Err(err) = obj.run(event_queue, sender).await {
                log::error!("Unexpected error reading wayland socket: {err}");
            }
        }));

        receiver.attach(
            None,
            glib::clone!(@weak self as obj => @default-return glib::Continue(false), move |transform| {
                obj.set_transform(transform);

                glib::Continue(true)
            }));

        Ok(())
    }

    async fn run(
        &self,
        mut event_queue: EventQueue<State>,
        sender: glib::Sender<crate::Transform>,
    ) -> anyhow::Result<()> {
        poll_fn(|cx| {
            let mut state = State::default();
            let res = event_queue.poll_dispatch_pending(cx, &mut state);

            if let Some(transform) = state.transform {
                let _ = sender.send(transform);
            }

            res
        })
        .await?;

        Ok(())
    }
}

impl wayland_client::Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &wayland_client::Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface.as_str() == "wl_output" {
                state.supports_output = true;
                registry.bind::<wl_output::WlOutput, (), State>(
                    name,
                    version.min(WL_OUTPUT_V1),
                    qhandle,
                    (),
                );
            }
        }
    }
}

impl wayland_client::Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        _registry: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &wayland_client::Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Geometry {
            transform,
            make,
            model,
            ..
        } = event
        {
            if let WEnum::Value(transform) = transform {
                let transform: crate::Transform = transform.into();
                log::debug!("FOUND {transform:?} for {model}, {make}");
                state.transform = Some(transform);
            }
        }
    }
}

fn prepare_read(conn: &wayland_client::Connection) -> anyhow::Result<()> {
    conn.prepare_read()?.read()?;
    Ok(())
}
