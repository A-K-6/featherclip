use crate::{clipboard::Clipboard, history::History};
use std::{
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter};

pub struct Shared {
    pub history: History,
    pub error: Option<String>,
    pub shortcut: bool,
}

pub enum Action {
    Copy(u32),
    TogglePause,
    Clear,
}
type Reply = mpsc::SyncSender<Result<(), String>>;

pub struct Worker {
    pub shared: Arc<Mutex<Shared>>,
    sender: mpsc::Sender<(Action, Reply)>,
}

impl Worker {
    pub fn start(app: AppHandle, demo: bool) -> Self {
        let mut history = History::new();
        if demo {
            for text in [
                "Hello from FeatherClip 👋",
                "A tiny home for the things you copy.",
                "سلام! این یک کلیپ آزمایشی است.",
            ] {
                history.insert(text.into());
            }
        }
        let shared = Arc::new(Mutex::new(Shared {
            history,
            error: None,
            shortcut: false,
        }));
        let (sender, receiver) = mpsc::channel::<(Action, Reply)>();
        let worker_state = Arc::clone(&shared);
        std::thread::spawn(move || {
            let mut clipboard = Clipboard::new(demo);
            // Establish a baseline before processing user actions.
            poll(&app, &worker_state, &mut clipboard);
            loop {
                match receiver.recv_timeout(Duration::from_millis(750)) {
                    Ok((action, reply)) => {
                        let result = handle_action(&worker_state, &mut clipboard, action);
                        let _ = reply.send(result);
                        let _ = app.emit("history-changed", ());
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        poll(&app, &worker_state, &mut clipboard)
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Self { shared, sender }
    }

    pub async fn action(&self, action: Action) -> Result<(), String> {
        let sender = self.sender.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let (reply, result) = mpsc::sync_channel(1);
            sender
                .send((action, reply))
                .map_err(|_| "Clipboard worker stopped.".to_string())?;
            result
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| "Clipboard is busy. Please try again.".to_string())?
        })
        .await
        .map_err(|_| "Clipboard worker couldn't complete this action.".to_string())?
    }
}

fn handle_action(
    shared: &Mutex<Shared>,
    clipboard: &mut Result<Clipboard, String>,
    action: Action,
) -> Result<(), String> {
    match action {
        Action::Copy(id) => {
            let text = shared
                .lock()
                .unwrap()
                .history
                .items
                .iter()
                .find(|clip| clip.id == id)
                .map(|clip| clip.text.clone())
                .ok_or("This clip is no longer in history.")?;
            clipboard
                .as_mut()
                .map_err(|e| e.clone())?
                .write(text.clone())?;
            let mut state = shared.lock().unwrap();
            state.history.copied(text);
            state.error = None;
        }
        Action::TogglePause => {
            shared.lock().unwrap().history.toggle_pause();
            establish_baseline(shared, clipboard);
        }
        Action::Clear => {
            shared.lock().unwrap().history.clear();
            establish_baseline(shared, clipboard);
        }
    }
    Ok(())
}

fn establish_baseline(shared: &Mutex<Shared>, clipboard: &mut Result<Clipboard, String>) {
    if shared.lock().unwrap().history.paused {
        return;
    }
    if let Ok(clipboard) = clipboard {
        clipboard.changed();
        if let Ok(text) = clipboard.read() {
            shared.lock().unwrap().history.observe(text);
        }
    }
}

fn poll(app: &AppHandle, shared: &Mutex<Shared>, clipboard: &mut Result<Clipboard, String>) {
    if shared.lock().unwrap().history.paused {
        return;
    }
    if clipboard.is_err() {
        *clipboard = Clipboard::new(false);
    }
    if clipboard
        .as_mut()
        .is_ok_and(|clipboard| !clipboard.changed())
    {
        return;
    }
    // Don't hold the state lock while another app supplies clipboard data.
    let observation = clipboard
        .as_mut()
        .map_err(|error| error.clone())
        .and_then(Clipboard::read);
    let mut state = shared.lock().unwrap();
    let old_error = state.error.clone();
    let changed = match observation {
        Ok(text) => {
            state.error = None;
            state.history.observe(text)
        }
        Err(error) => {
            state.error = Some(error);
            false
        }
    };
    if changed || old_error != state.error {
        let _ = app.emit("history-changed", ());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_and_resume_capture_the_very_next_copy() {
        let shared = Mutex::new(Shared {
            history: History::new(),
            error: None,
            shortcut: false,
        });
        let mut clipboard = Ok(Clipboard::Demo(Some("current".into())));
        handle_action(&shared, &mut clipboard, Action::Clear).unwrap();
        assert!(shared
            .lock()
            .unwrap()
            .history
            .observe(Some("new after clear".into())));
        handle_action(&shared, &mut clipboard, Action::TogglePause).unwrap();
        handle_action(&shared, &mut clipboard, Action::TogglePause).unwrap();
        assert!(shared
            .lock()
            .unwrap()
            .history
            .observe(Some("new after resume".into())));
    }

    #[test]
    fn worker_copies_selected_text_and_rejects_stale_ids() {
        let mut history = History::new();
        history.insert("first".into());
        let id = history.items[0].id;
        history.insert("second".into());
        let shared = Mutex::new(Shared {
            history,
            error: None,
            shortcut: false,
        });
        let mut clipboard = Ok(Clipboard::Demo(None));
        handle_action(&shared, &mut clipboard, Action::Copy(id)).unwrap();
        assert_eq!(
            clipboard.as_mut().unwrap().read().unwrap().as_deref(),
            Some("first")
        );
        assert_eq!(shared.lock().unwrap().history.items[0].id, id);
        handle_action(&shared, &mut clipboard, Action::TogglePause).unwrap();
        assert!(shared.lock().unwrap().history.paused);
        handle_action(&shared, &mut clipboard, Action::Clear).unwrap();
        assert!(handle_action(&shared, &mut clipboard, Action::Copy(id)).is_err());
        assert_eq!(
            clipboard.as_mut().unwrap().read().unwrap().as_deref(),
            Some("first")
        );
    }
}
