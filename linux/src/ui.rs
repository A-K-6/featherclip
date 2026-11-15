use crate::{
    content::Kind,
    paste,
    worker::{Action, Worker},
};
use adw::prelude::*;
use gtk::gdk;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

thread_local! { static UI: RefCell<Option<Rc<Ui>>> = const { RefCell::new(None) }; }
fn on_ui(f: impl FnOnce(&Rc<Ui>) + Send + 'static) {
    glib::idle_add_once(move || {
        UI.with(|slot| {
            if let Some(ui) = slot.borrow().as_ref() {
                f(ui);
            }
        })
    });
}
struct Ui {
    window: adw::ApplicationWindow,
    search: gtk::SearchEntry,
    list: gtk::ListBox,
    status: gtk::Label,
    hint: gtk::Label,
    pause: gtk::Button,
    clear: gtk::Button,
    worker: Worker,
    ids: RefCell<Vec<u32>>,
    target: Cell<paste::Target>,
    generation: Cell<u64>,
    busy: Cell<bool>,
    focused: Cell<bool>,
    demo: bool,
    shortcut: RefCell<Option<global_hotkey::GlobalHotKeyManager>>,
}
impl Ui {
    fn new(app: &adw::Application, demo: bool) -> Rc<Self> {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("FeatherClip")
            .default_width(360)
            .default_height(430)
            .resizable(false)
            .decorated(false)
            .build();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.set_margin_top(14);
        root.set_margin_bottom(14);
        root.set_margin_start(14);
        root.set_margin_end(14);
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let title = gtk::Label::new(Some("Clipboard"));
        title.add_css_class("heading");
        title.set_hexpand(true);
        title.set_xalign(0.0);
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("flat");
        close.set_tooltip_text(Some("Close (Esc)"));
        heading.append(&title);
        heading.append(&close);
        root.append(&heading);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search clips"));
        root.append(&search);
        let status = gtk::Label::new(Some("Listening"));
        status.add_css_class("dim-label");
        status.add_css_class("caption");
        status.set_xalign(0.0);
        status.set_wrap(true);
        root.append(&status);
        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.set_activate_on_single_click(true);
        let empty = gtk::Label::new(Some("Copy something to get started"));
        empty.add_css_class("dim-label");
        list.set_placeholder(Some(&empty));
        let scroll = gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .min_content_height(210)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        root.append(&scroll);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let pause = gtk::Button::with_label("Pause");
        let clear = gtk::Button::with_label("Clear");
        pause.add_css_class("flat");
        clear.add_css_class("flat");
        footer.append(&pause);
        footer.append(&clear);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        footer.append(&spacer);
        let menu = gio::Menu::new();
        menu.append(Some("Start at login"), Some("app.login"));
        if crate::clipboard::needs_gnome_bridge() {
            menu.append(Some("Enable GNOME integration"), Some("app.gnome"));
        }
        menu.append(Some("Quit FeatherClip"), Some("app.quit"));
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .menu_model(&menu)
            .build();
        more.add_css_class("flat");
        footer.append(&more);
        root.append(&footer);
        let hint = gtk::Label::new(Some("Enter to paste · Shift+Enter to copy"));
        hint.add_css_class("caption");
        hint.add_css_class("dim-label");
        hint.set_wrap(true);
        hint.set_xalign(0.0);
        root.append(&hint);
        window.set_content(Some(&root));
        let worker = Worker::start(demo, || on_ui(|ui| ui.refresh()));
        let ui = Rc::new(Self {
            window,
            search,
            list,
            status,
            hint,
            pause,
            clear,
            worker,
            ids: RefCell::new(Vec::new()),
            target: Cell::new(paste::Target::None),
            generation: Cell::new(0),
            busy: Cell::new(false),
            focused: Cell::new(false),
            demo,
            shortcut: RefCell::new(None),
        });
        close.connect_clicked(|_| on_ui(|ui| ui.hide()));
        ui.search
            .connect_search_changed(|_| on_ui(|ui| ui.refresh()));
        ui.search.connect_activate(|_| on_ui(|ui| ui.choose(false)));
        ui.list
            .connect_row_activated(|_, _| on_ui(|ui| ui.choose(false)));
        ui.pause
            .connect_clicked(|_| on_ui(|ui| ui.action(Action::TogglePause)));
        ui.clear
            .connect_clicked(|_| on_ui(|ui| ui.action(Action::Clear)));
        ui.window.connect_close_request(|_| {
            on_ui(|ui| ui.hide());
            glib::Propagation::Stop
        });
        ui.window.connect_is_active_notify(|window| {
            let active = window.is_active();
            on_ui(move |ui| {
                if active {
                    ui.focused.set(true);
                } else if ui.focused.get() && !ui.demo {
                    ui.hide();
                }
            });
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(|_, key, _, modifiers| match key {
            gdk::Key::Escape => {
                on_ui(|ui| ui.hide());
                glib::Propagation::Stop
            }
            gdk::Key::Return | gdk::Key::KP_Enter => {
                let copy = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
                on_ui(move |ui| ui.choose(copy));
                glib::Propagation::Stop
            }
            gdk::Key::Down | gdk::Key::Up => {
                on_ui(move |ui| {
                    let count = ui.ids.borrow().len() as i32;
                    if count == 0 {
                        return;
                    }
                    let current = ui.list.selected_row().map(|r| r.index()).unwrap_or(-1);
                    let next = if key == gdk::Key::Down {
                        (current + 1).min(count - 1)
                    } else {
                        (current - 1).max(0)
                    };
                    if let Some(row) = ui.list.row_at_index(next) {
                        ui.list.select_row(Some(&row));
                        row.grab_focus();
                    }
                });
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        });
        ui.window.add_controller(keys);
        let quit = gio::SimpleAction::new("quit", None);
        let weak = app.downgrade();
        quit.connect_activate(move |_, _| {
            if let Some(app) = weak.upgrade() {
                app.quit();
            }
        });
        app.add_action(&quit);
        let login = gio::SimpleAction::new_stateful(
            "login",
            None,
            &autostart_path().is_some_and(|p| p.exists()).to_variant(),
        );
        login.set_enabled(!demo);
        login.connect_activate(|action, _| {
            let enabled = !action
                .state()
                .and_then(|v| v.get::<bool>())
                .unwrap_or(false);
            match set_autostart(enabled) {
                Ok(()) => action.set_state(&enabled.to_variant()),
                Err(e) => on_ui(move |ui| ui.hint.set_text(&e)),
            }
        });
        app.add_action(&login);
        let gnome = gio::SimpleAction::new("gnome", None);
        gnome.set_enabled(!demo);
        gnome.connect_activate(|_, _| {
            std::thread::spawn(|| {
                let _ = std::process::Command::new("gnome-extensions")
                    .args(["disable", "featherclip@aeen.github"])
                    .status();
                let result = std::process::Command::new("gnome-extensions")
                    .args(["enable", "featherclip@aeen.github"])
                    .status();
                let text = if result.is_ok_and(|s| s.success()) {
                    "GNOME integration enabled. If this is the first install, log out and back in."
                } else {
                    "Log out and back in, then enable FeatherClip in Extensions."
                };
                on_ui(move |ui| ui.hint.set_text(text));
            });
        });
        app.add_action(&gnome);
        if !demo {
            if std::env::var_os("WAYLAND_DISPLAY").is_none() {
                use global_hotkey::{
                    hotkey::{Code, HotKey, Modifiers},
                    GlobalHotKeyEvent, HotKeyState,
                };
                if let Ok(manager) = global_hotkey::GlobalHotKeyManager::new() {
                    if manager
                        .register(HotKey::new(
                            Some(Modifiers::CONTROL | Modifiers::ALT),
                            Code::KeyV,
                        ))
                        .is_ok()
                    {
                        GlobalHotKeyEvent::set_event_handler(Some(|event: GlobalHotKeyEvent| {
                            if event.state == HotKeyState::Pressed {
                                on_ui(|ui| ui.toggle());
                            }
                        }));
                        *ui.shortcut.borrow_mut() = Some(manager);
                    }
                }
            }
            use ksni::blocking::TrayMethods;
            if let Ok(handle) = Tray.spawn() {
                std::mem::forget(handle);
            }
        }
        ui
    }
    fn toggle(&self) {
        if self.window.is_visible() {
            self.hide();
        } else {
            self.show();
        }
    }
    fn show(&self) {
        self.generation.set(self.generation.get() + 1);
        self.target.set(if self.demo {
            paste::Target::None
        } else {
            paste::remember()
        });
        self.focused.set(false);
        self.search.set_text("");
        self.refresh_force();
        self.window.present();
        self.search.grab_focus();
        if matches!(self.target.get(), paste::Target::Gnome) {
            glib::timeout_add_local_once(std::time::Duration::from_millis(120), || {
                let _ = paste::gnome_call("FocusPopup", Some(&(std::process::id(),).to_variant()));
            });
        }
    }
    fn hide(&self) {
        self.window.set_visible(false);
        self.generation.set(self.generation.get() + 1);
        self.list.remove_all();
        self.ids.borrow_mut().clear();
    }
    fn refresh(&self) {
        if self.window.is_visible() {
            self.refresh_force();
        }
    }
    fn refresh_force(&self) {
        let selected = self
            .list
            .selected_row()
            .and_then(|r| self.ids.borrow().get(r.index() as usize).copied());
        self.list.remove_all();
        self.ids.borrow_mut().clear();
        let query = self.search.text().to_lowercase();
        let state = self.worker.shared.lock().unwrap();
        self.status
            .set_text(state.error.as_deref().unwrap_or(&format!(
                "{} / 30 clips · {}",
                state.history.items.len(),
                if state.history.paused {
                    "Paused"
                } else {
                    "Listening"
                }
            )));
        self.pause.set_label(if state.history.paused {
            "Resume"
        } else {
            "Pause"
        });
        self.clear.set_sensitive(!state.history.items.is_empty());
        for clip in state
            .history
            .items
            .iter()
            .filter(|clip| query.is_empty() || clip.text.to_lowercase().contains(&query))
        {
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            content.set_margin_top(10);
            content.set_margin_bottom(10);
            content.set_margin_start(10);
            content.set_margin_end(10);
            if let Some(texture) = clip
                .preview
                .as_ref()
                .and_then(|p| gdk::Texture::from_bytes(&glib::Bytes::from(p.as_slice())).ok())
            {
                let picture = gtk::Picture::for_paintable(&texture);
                picture.set_size_request(44, 36);
                picture.set_content_fit(gtk::ContentFit::Contain);
                content.append(&picture);
            } else if clip.kind != Kind::Text {
                let icon = gtk::Image::from_icon_name(if clip.kind == Kind::Files {
                    "text-x-generic-symbolic"
                } else {
                    "image-x-generic-symbolic"
                });
                icon.set_pixel_size(32);
                content.append(&icon);
            }
            let text = gtk::Label::new(Some(&clip.text.chars().take(400).collect::<String>()));
            text.set_xalign(0.0);
            text.set_hexpand(true);
            text.set_wrap(true);
            text.set_lines(2);
            text.set_max_width_chars(32);
            text.set_ellipsize(gtk::pango::EllipsizeMode::End);
            content.append(&text);
            let row = gtk::ListBoxRow::new();
            row.set_child(Some(&content));
            self.list.append(&row);
            self.ids.borrow_mut().push(clip.id);
            if Some(clip.id) == selected {
                self.list.select_row(Some(&row));
            }
        }
    }
    fn action(&self, action: Action) {
        self.worker.action(action, |result| {
            if let Err(e) = result {
                on_ui(move |ui| ui.hint.set_text(&e));
            }
        });
    }
    fn choose(&self, copy_only: bool) {
        if self.busy.get() {
            return;
        }
        let index = self
            .list
            .selected_row()
            .map(|r| r.index() as usize)
            .unwrap_or(0);
        let Some(id) = self.ids.borrow().get(index).copied() else {
            return;
        };
        self.busy.set(true);
        let generation = self.generation.get();
        let target = self.target.get();
        self.worker.action(Action::Copy(id), move |result| {
            on_ui(move |ui| {
                ui.busy.set(false);
                if ui.generation.get() != generation {
                    return;
                }
                if let Err(e) = result {
                    ui.hint.set_text(&e);
                    return;
                }
                if copy_only || ui.demo {
                    ui.hint.set_text("Copied");
                    return;
                }
                if !target.available() {
                    ui.hint
                        .set_text("Copied · Press Ctrl+V in your destination app");
                    return;
                }
                ui.hide();
                std::thread::spawn(move || {
                    if let Err(e) = paste::perform(target) {
                        on_ui(move |ui| ui.hint.set_text(&e));
                    }
                });
            })
        });
    }
}
struct Tray;
impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "featherclip".into()
    }
    fn title(&self) -> String {
        "FeatherClip".into()
    }
    fn icon_name(&self) -> String {
        "edit-paste".into()
    }
    fn activate(&mut self, _: i32, _: i32) {
        on_ui(|ui| ui.toggle());
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Open clipboard".into(),
                activate: Box::new(|_| on_ui(|ui| ui.toggle())),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit FeatherClip".into(),
                activate: Box::new(|_| {
                    on_ui(|ui| {
                        if let Some(app) = ui.window.application() {
                            app.quit();
                        }
                    })
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
fn autostart_path() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|v| std::path::PathBuf::from(v).join(".config")))
        .map(|p| p.join("autostart/featherclip.desktop"))
}
fn set_autostart(enabled: bool) -> Result<(), String> {
    let path = autostart_path().ok_or("Couldn't find your configuration folder.")?;
    if enabled {
        let entry = concat!(
            "[Desktop Entry]\n",
            "Type=Application\n",
            "Name=FeatherClip\n",
            "Exec=/usr/bin/featherclip --background\n",
            "Icon=featherclip\n",
            "X-GNOME-Autostart-enabled=true\n",
        );
        std::fs::create_dir_all(path.parent().unwrap())
            .and_then(|_| std::fs::write(&path, entry))
            .map_err(|_| "Couldn't save the login item.".to_string())
    } else {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("Couldn't remove the login item.".into()),
        }
    }
}
pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("io.github.AK6.FeatherClip")
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    for option in ["toggle", "background", "demo", "smoke-test"] {
        app.add_main_option(
            option,
            glib::Char::from(0),
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            option,
            None,
        );
    }
    app.connect_command_line(|app, command| {
        let options = command.options_dict();
        let smoke = options.contains("smoke-test");
        let demo = smoke || options.contains("demo");
        UI.with(|slot| {
            if slot.borrow().is_none() {
                let ui = Ui::new(app, demo);
                *slot.borrow_mut() = Some(ui);
                std::mem::forget(app.hold());
            }
        });
        UI.with(|slot| {
            let slot = slot.borrow();
            let ui = slot.as_ref().unwrap();
            if options.contains("toggle") {
                ui.toggle();
            } else if !options.contains("background") {
                ui.show();
            }
        });
        if smoke {
            start_smoke(app);
        }
        0
    });
    app.run()
}
fn start_smoke(app: &adw::Application) {
    let app = app.clone();
    let phase = Cell::new(0);
    let attempts = Cell::new(0);
    glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
        attempts.set(attempts.get() + 1);
        assert!(attempts.get() < 100, "native GTK smoke timed out");
        UI.with(|slot| {
            let slot = slot.borrow();
            let ui = slot.as_ref().unwrap();
            match phase.get() {
                0 if ui.ids.borrow().len() == 5 => {
                    assert!(ui.window.is_visible());
                    ui.search.set_text("سلام");
                    ui.refresh_force();
                    assert_eq!(ui.ids.borrow().len(), 1);
                    ui.choose(true);
                    ui.action(Action::TogglePause);
                    phase.set(1);
                }
                1 if ui.worker.shared.lock().unwrap().history.paused => {
                    ui.action(Action::Clear);
                    phase.set(2);
                }
                2 if ui.worker.shared.lock().unwrap().history.items.is_empty() => {
                    ui.hide();
                    ui.show();
                    assert!(ui.window.is_visible());
                    assert!(ui.ids.borrow().is_empty());
                    println!("SMOKE PASS: native GTK/libadwaita, Unicode search, copy, pause, clear, popup");
                    phase.set(3);
                    app.quit();
                }
                _ => {}
            }
        });
        if phase.get() == 3 {
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}
