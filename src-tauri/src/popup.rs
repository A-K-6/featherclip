use tauri::WebviewWindow;

pub fn dismiss(window: &WebviewWindow) -> tauri::Result<()> {
    // Tauri/WebKitGTK loses private URI handlers when an incognito webview is
    // rebuilt. Reuse one private view on Linux; its hidden UI stops refreshing.
    if cfg!(target_os = "linux") {
        window.hide()
    } else {
        window.close()
    }
}

/// Configure before showing: a utility popup belongs over the user's current
/// Space, including another app's full-screen Space, and never joins window tabs.
pub fn prepare(window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    if let Ok(pointer) = window.ns_window() {
        use objc2_app_kit::{
            NSWindow, NSWindowCollectionBehavior as Behavior, NSWindowTabbingMode,
        };
        // Called on the main thread; Tauri owns the live NSWindow for this call.
        let native = unsafe { &*pointer.cast::<NSWindow>() };
        native.setCollectionBehavior(
            Behavior::CanJoinAllSpaces
                | Behavior::FullScreenAuxiliary
                | Behavior::Transient
                | Behavior::IgnoresCycle,
        );
        native.setTabbingMode(NSWindowTabbingMode::Disallowed);
    }

    // Open on the display the user is working on, rather than always the primary.
    if let Ok(cursor) = window.cursor_position() {
        if let Ok(Some(monitor)) = window.monitor_from_point(cursor.x, cursor.y) {
            if let Ok(size) = window.outer_size() {
                let area = monitor.work_area();
                let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
                let y = area.position.y + (area.size.height as i32 - size.height as i32) / 2;
                let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
            }
        }
    }
}

pub fn check(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{
            NSWindow, NSWindowCollectionBehavior as Behavior, NSWindowTabbingMode,
        };
        let pointer = window.ns_window().map_err(|e| e.to_string())?;
        let native = unsafe { &*pointer.cast::<NSWindow>() };
        if !native
            .collectionBehavior()
            .contains(Behavior::CanJoinAllSpaces | Behavior::FullScreenAuxiliary)
            || native.tabbingMode() != NSWindowTabbingMode::Disallowed
        {
            return Err("Popup lost its macOS Spaces or tabbing configuration.".into());
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
    Ok(())
}
