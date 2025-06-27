use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::CompositeTemplate;
use gtk::glib;

mod imp {
    use super::*;

    use glib::Properties;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/org/gnome/Snapshot/ui/qr_bottom_sheet.ui")]
    #[properties(wrapper_type = super::QrBottomSheet)]
    pub struct QrBottomSheet {
        #[template_child]
        pub action_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub open_external_button: TemplateChild<gtk::Button>,

        #[property(get, set = Self::set_contents, explicit_notify)]
        pub contents: RefCell<Option<String>>,
    }

    impl QrBottomSheet {
        fn set_contents(&self, contents: String) {
            if self
                .contents
                .borrow()
                .as_ref()
                .is_some_and(|old| old == &contents)
            {
                return;
            }
            self.action_row.set_subtitle(&contents);

            let is_url = contents.starts_with("https://") || contents.starts_with("http://");
            self.open_external_button.set_visible(is_url);

            self.contents.replace(Some(contents));

            self.obj().notify_contents();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for QrBottomSheet {
        const NAME: &'static str = "QrBottomSheet";
        type Type = super::QrBottomSheet;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("qr-bottom-sheet.copy", None, |widget, _, _| {
                widget.copy();
            });
            klass.install_action_async(
                "qr-bottom-sheet.open-external",
                None,
                async |widget, _, _| {
                    if let Err(err) = widget.open_external().await {
                        log::error!("Could not open external URI: {err}");
                    }
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for QrBottomSheet {
        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for QrBottomSheet {}
    impl BinImpl for QrBottomSheet {}
}

glib::wrapper! {
    pub struct QrBottomSheet(ObjectSubclass<imp::QrBottomSheet>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible;
}

impl Default for QrBottomSheet {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl QrBottomSheet {
    fn copy(&self) {
        if let Some(contents) = self.imp().contents.borrow().as_ref() {
            self.clipboard().set_text(contents);
            let root = self.root();
            let window = root.and_downcast_ref::<crate::Window>().unwrap();
            window.send_toast(&gettext("Copied to clipboard"));
        }
    }

    async fn open_external(&self) -> Result<(), glib::Error> {
        let imp = self.imp();

        let launcher = if let Some(uri) = imp.contents.borrow().as_ref() {
            gtk::UriLauncher::new(uri)
        } else {
            return Ok(());
        };

        let root = self.root();
        let window = root.and_downcast_ref::<gtk::Window>();
        let res = launcher.launch_future(window).await;

        if let Err(ref err) = res
            && err.matches(gtk::DialogError::Dismissed)
        {
            return Ok(());
        }
        res?;

        Ok(())
    }
}
