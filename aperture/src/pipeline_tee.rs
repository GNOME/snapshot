// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::HashMap;

use gst::prelude::*;
use gst::subclass::prelude::*;
use gtk::glib;

mod imp {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    #[derive(Debug, Default)]
    pub struct PipelineTee {
        pub hashmap: Mutex<HashMap<gst::Element, gst::Element>>,
        pub tee: OnceLock<gst::Element>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PipelineTee {
        const NAME: &'static str = "AperturePipelineTee";
        type Type = super::PipelineTee;
        type ParentType = gst::Bin;
    }

    impl ObjectImpl for PipelineTee {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let tee = gst::ElementFactory::make("tee").build().unwrap();
            obj.add(&tee).unwrap();

            let pad = tee.static_pad("sink").unwrap();
            let ghost_pad = gst::GhostPad::with_target(&pad).unwrap();
            ghost_pad.set_active(true).unwrap();

            obj.add_pad(&ghost_pad).unwrap();

            self.tee.set(tee).unwrap();
        }
    }

    impl GstObjectImpl for PipelineTee {}
    impl ElementImpl for PipelineTee {}
    impl BinImpl for PipelineTee {}
}

glib::wrapper! {
    pub struct PipelineTee(ObjectSubclass<imp::PipelineTee>)
        @extends gst::Bin, gst::Element, gst::Object;
}

impl Default for PipelineTee {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl PipelineTee {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_branch(&self, branch: &gst::Element) {
        let imp = self.imp();

        let queue = gst::ElementFactory::make("queue").build().unwrap();

        imp.hashmap
            .lock()
            .unwrap()
            .insert(branch.clone(), queue.clone());

        self.add_many([&queue, branch]).unwrap();
        queue.link(branch).unwrap();

        let tee_pad = imp.tee.get().unwrap().request_pad_simple("src_%u").unwrap();
        let queue_pad = queue.static_pad("sink").unwrap();

        tee_pad.link(&queue_pad).unwrap();

        queue.sync_state_with_parent().unwrap();
        branch.sync_state_with_parent().unwrap();
    }

    pub fn remove_branch(&self, branch: &gst::Element) {
        let imp = self.imp();
        if let Some(queue) = imp.hashmap.lock().unwrap().remove(branch) {
            let queue_pad = queue.static_pad("sink").unwrap();
            let tee_pad = queue_pad.peer().unwrap();

            tee_pad.add_probe(
                gst::PadProbeType::BLOCK_DOWNSTREAM,
                glib::clone!(
                    #[weak(rename_to = obj)]
                    self,
                    #[weak]
                    branch,
                    #[weak]
                    queue,
                    #[upgrade_or]
                    gst::PadProbeReturn::Remove,
                    move |tee_pad, _| {
                        let tee = obj.imp().tee.get().unwrap();
                        tee.call_async(glib::clone!(
                            #[weak]
                            obj,
                            #[weak]
                            tee_pad,
                            #[weak]
                            branch,
                            #[weak]
                            queue,
                            move |tee| {
                                tee.release_request_pad(&tee_pad);

                                branch.set_state(gst::State::Null).unwrap();
                                queue.set_state(gst::State::Null).unwrap();

                                obj.remove(&queue).unwrap();
                                obj.remove(&branch).unwrap();
                            }
                        ));

                        gst::PadProbeReturn::Remove
                    }
                ),
            );
        } else {
            log::error!("Branch {branch:?} not in the pipeline");
        }
    }
}
