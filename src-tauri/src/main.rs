#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod history;
mod paste;
mod worker;

use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use worker::{Action, Worker};

struct Mode {
    demo: bool,
    smoke: bool,
}

#[derive(Serialize)]
struct Snapshot {
    items: Vec<history::Clip>,
    paused: bool,
    error: Option<String>,
    gnome: bool,
    shortcut: bool,
    autostart: bool,
    mac: bool,
    paste_permission: bool,
    paste_target: bool,
}

#[tauri::command]
fn snapshot(app: AppHandle, worker: tauri::State<'_, Worker>) -> Snapshot {
    let state = worker.shared.lock().unwrap();
    Snapshot {
        items: state.history.items.clone(),
        paused: state.history.paused,
        error: state.error.clone(),
        gnome: clipboard::needs_gnome_bridge(),
        shortcut: state.shortcut,
        mac: cfg!(target_os = "macos"),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        paste_permission: paste::access_allowed(),
        paste_target: app
            .state::<paste::Controller>()
            .target
            .lock()
            .unwrap()
            .available(),
    }
}

#[tauri::command]
async fn copy_clip(worker: tauri::State<'_, Worker>, id: u32) -> Result<(), String> {
    worker.action(Action::Copy(id)).await
}

#[tauri::command]
async fn select_clip(app: AppHandle, id: u32, copy_only: bool) -> Result<&'static str, String> {
    use std::sync::atomic::Ordering;
    let controller = app.state::<paste::Controller>();
    let popup = app.get_webview_window("history");
    if controller.busy.swap(true, Ordering::AcqRel) {
        return Err("A clip is already being selected.".into());
    }
    struct Release<'a>(&'a std::sync::atomic::AtomicBool);
    impl Drop for Release<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }
    let _release = Release(&controller.busy);
    app.state::<Worker>().action(Action::Copy(id)).await?;
    if copy_only || app.state::<Mode>().demo {
        return Ok("copied");
    }
    if !paste::access_allowed() {
        return Ok("permission");
    }
    let target = std::mem::take(&mut *controller.target.lock().unwrap());
    tauri::async_runtime::spawn_blocking(move || paste::perform(target))
        .await
        .map_err(|_| "Copied, but Paste couldn't finish. Paste manually.".to_string())??;
    if let Some(popup) = popup {
        let _ = popup.close();
    }
    Ok("pasted")
}

#[tauri::command]
fn request_paste_permission(app: AppHandle) -> bool {
    !app.state::<Mode>().demo && paste::request_access()
}

#[tauri::command]
async fn toggle_pause(worker: tauri::State<'_, Worker>) -> Result<(), String> {
    worker.action(Action::TogglePause).await
}

#[tauri::command]
async fn clear_history(worker: tauri::State<'_, Worker>) -> Result<(), String> {
    worker.action(Action::Clear).await
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    if app.state::<Mode>().demo {
        return Err("Start at login is disabled in demo mode.".into());
    }
    let result = if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    };
    result.map_err(|_| {
        "Couldn't change launch at login. Install FeatherClip first, then try again.".into()
    })
}

#[tauri::command]
fn enable_gnome() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("gnome-extensions")
            .args(["enable", "featherclip@aeen.github"])
            .output()
            .map_err(|_| "GNOME Extensions isn't available on this desktop.".to_string())?;
        if output.status.success() {
            return Ok(());
        }
        Err("GNOME hasn't discovered the bridge yet. Log out and back in, reopen FeatherClip, then enable it here. Ubuntu 24.04 or newer is required for GNOME Wayland.".into())
    }
    #[cfg(not(target_os = "linux"))]
    Err("The GNOME bridge is only needed on Ubuntu/GNOME Wayland.".into())
}

#[tauri::command]
fn close_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("history") {
        let _ = window.close();
    }
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn test_mode(app: AppHandle) -> bool {
    app.state::<Mode>().smoke
}

#[tauri::command]
fn finish_smoke_test(app: AppHandle, success: bool) -> Result<(), String> {
    if !app.state::<Mode>().smoke {
        return Err("Not in smoke-test mode.".into());
    }
    if success {
        println!("SMOKE PASS: webview, IPC, synthetic history, copy, pause, clear");
    } else {
        eprintln!("SMOKE FAIL: frontend acceptance check");
    }
    app.exit(if success { 0 } else { 1 });
    Ok(())
}

fn show_history(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = handle.get_webview_window("history") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
            return;
        }
        let demo = handle.state::<Mode>().demo;
        *handle.state::<paste::Controller>().target.lock().unwrap() = if demo {
            paste::Target::None
        } else {
            paste::remember()
        };
        // Create only when opened. Closing destroys the webview, releasing its resources.
        if let Ok(window) =
            WebviewWindowBuilder::new(&handle, "history", WebviewUrl::App("index.html".into()))
                .title("FeatherClip")
                .incognito(true)
                .inner_size(360.0, 420.0)
                .decorations(false)
                .resizable(false)
                .skip_taskbar(true)
                .always_on_top(true)
                .center()
                .build()
        {
            if !demo {
                let popup = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = popup.close();
                    }
                });
            }
            let _ = window.set_focus();
        }
    });
}

fn toggle_history(app: &AppHandle) {
    if app.get_webview_window("history").is_some() {
        close_window(app.clone());
    } else {
        show_history(app);
    }
}

fn main() {
    let smoke = std::env::args().any(|arg| arg == "--smoke-test");
    let demo = smoke || std::env::args().any(|arg| arg == "--demo");
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if args.iter().any(|arg| arg == "--toggle") {
                toggle_history(app);
            } else {
                show_history(app);
            }
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_history(app);
                    }
                })
                .build(),
        )
        .manage(Mode { demo, smoke })
        .manage(paste::Controller::default())
        .invoke_handler(tauri::generate_handler![
            snapshot,
            copy_clip,
            select_clip,
            request_paste_permission,
            toggle_pause,
            clear_history,
            set_autostart,
            enable_gnome,
            close_window,
            quit,
            test_mode,
            finish_smoke_test
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let worker = Worker::start(app.handle().clone(), demo);
            let shortcut = if cfg!(target_os = "macos") {
                "Command+Shift+V"
            } else {
                "Control+Alt+V"
            };
            // X11 shortcut APIs don't register desktop-wide shortcuts in native Wayland sessions.
            let wayland =
                cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some();
            if !demo && !wayland {
                worker.shared.lock().unwrap().shortcut =
                    app.global_shortcut().register(shortcut).is_ok();
            }
            app.manage(worker);
            let open =
                MenuItem::with_id(app, "open", "Open Clipboard History", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit FeatherClip", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            TrayIconBuilder::with_id("featherclip")
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/tray.png"
                ))?)
                .icon_as_template(cfg!(target_os = "macos"))
                .tooltip("FeatherClip — clipboard history")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_history(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            show_history(app.handle());
            if smoke {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(25));
                    eprintln!("SMOKE FAIL: webview timed out");
                    handle.exit(1);
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Couldn't start FeatherClip")
        .run(|_, event| {
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                api.prevent_exit(); // Closing the history window leaves only the tray and Rust worker.
            }
        });
}
