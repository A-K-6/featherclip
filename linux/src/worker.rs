use crate::{clipboard::Clipboard, history::History};
use std::{
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};
pub struct Shared {
    pub history: History,
    pub error: Option<String>,
}
pub enum Action {
    Copy(u32),
    TogglePause,
    Clear,
}
type Reply = Box<dyn FnOnce(Result<(), String>) + Send>;
#[derive(Clone)]
pub struct Worker {
    pub shared: Arc<Mutex<Shared>>,
    sender: mpsc::Sender<(Action, Reply)>,
}
impl Worker {
    pub fn start(demo: bool, on_change: impl Fn() + Send + 'static) -> Self {
        let mut history = History::new();
        if demo {
            history.insert(crate::content::Content::Image {
                mime: "image/png",
                bytes: include_bytes!("../../assets/sample.png").to_vec().into(),
            });
            history
                .insert(crate::content::file_uris(b"file:///tmp/FeatherClip%20demo.txt").unwrap());
            for s in [
                "Hello from FeatherClip 👋",
                "A tiny home for the things you copy.",
                "سلام! این یک کلیپ آزمایشی است.",
            ] {
                history.insert(s.into());
            }
        }
        let shared = Arc::new(Mutex::new(Shared {
            history,
            error: None,
        }));
        let state = Arc::clone(&shared);
        let (sender, receiver) = mpsc::channel::<(Action, Reply)>();
        std::thread::spawn(move || {
            let mut clipboard = Clipboard::new(demo);
            poll(&state, &mut clipboard);
            on_change();
            loop {
                match receiver.recv_timeout(Duration::from_millis(750)) {
                    Ok((action, reply)) => {
                        let result = handle_action(&state, &mut clipboard, action);
                        reply(result);
                        on_change();
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if poll(&state, &mut clipboard) {
                            on_change();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Self { shared, sender }
    }
    pub fn action(&self, action: Action, reply: impl FnOnce(Result<(), String>) + Send + 'static) {
        if let Err(mpsc::SendError((_, reply))) = self.sender.send((action, Box::new(reply))) {
            reply(Err("Clipboard worker stopped.".into()));
        }
    }
}
fn poll(shared: &Mutex<Shared>, clipboard: &mut Result<Clipboard, String>) -> bool {
    if shared.lock().unwrap().history.paused {
        return false;
    }
    if clipboard.is_err() {
        *clipboard = Clipboard::new(false);
    }
    if clipboard.as_mut().is_ok_and(|c| !c.changed()) {
        return false;
    }
    let observation = clipboard
        .as_mut()
        .map_err(|e| e.clone())
        .and_then(Clipboard::read);
    let mut state = shared.lock().unwrap();
    let old = state.error.clone();
    let changed = match observation {
        Ok(v) => {
            state.error = None;
            state.history.observe(v)
        }
        Err(e) => {
            state.error = Some(e);
            false
        }
    };
    changed || old != state.error
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
                .map(|clip| clip.content.clone())
                .ok_or("This clip is no longer in history.")?;
            if let crate::content::Content::Files(uris) = &text {
                if !matches!(clipboard, Ok(Clipboard::Demo(_)))
                    && !uris.iter().all(|uri| {
                        url::Url::parse(uri)
                            .ok()
                            .and_then(|u| u.to_file_path().ok())
                            .is_some_and(|path| path.exists())
                    })
                {
                    return Err("A source file was moved, deleted, or is unavailable. Copy the files again.".into());
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_and_resume_capture_the_very_next_copy() {
        let shared = Mutex::new(Shared {
            history: History::new(),
            error: None,
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
        });
        let mut clipboard = Ok(Clipboard::Demo(None));
        handle_action(&shared, &mut clipboard, Action::Copy(id)).unwrap();
        assert_eq!(
            clipboard
                .as_mut()
                .unwrap()
                .read()
                .unwrap()
                .as_ref()
                .and_then(crate::content::Content::text),
            Some("first")
        );
        assert_eq!(shared.lock().unwrap().history.items[0].id, id);
        handle_action(&shared, &mut clipboard, Action::TogglePause).unwrap();
        assert!(shared.lock().unwrap().history.paused);
        handle_action(&shared, &mut clipboard, Action::Clear).unwrap();
        assert!(handle_action(&shared, &mut clipboard, Action::Copy(id)).is_err());
        assert_eq!(
            clipboard
                .as_mut()
                .unwrap()
                .read()
                .unwrap()
                .as_ref()
                .and_then(crate::content::Content::text),
            Some("first")
        );
    }
}
