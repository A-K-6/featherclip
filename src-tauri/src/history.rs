use serde::Serialize;

pub const CAPACITY: usize = 30;
pub const MAX_ITEM_BYTES: usize = 100 * 1024;

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct Clip {
    pub id: u32,
    pub text: String,
}

#[derive(Default)]
pub struct History {
    pub items: Vec<Clip>,
    pub paused: bool,
    last_seen: Option<String>,
    baseline_pending: bool,
    next_id: u32,
}

impl History {
    pub fn new() -> Self {
        Self {
            baseline_pending: true,
            ..Self::default()
        }
    }

    /// The first observation after launch, clear, or resume is a baseline, not a clip.
    pub fn observe(&mut self, text: Option<String>) -> bool {
        if self.paused {
            return false;
        }
        let text = text.filter(|s| s.len() <= MAX_ITEM_BYTES);
        let changed = text != self.last_seen;
        self.last_seen = text.clone();
        if self.baseline_pending {
            self.baseline_pending = false;
            return false;
        }
        if !changed {
            return false;
        }
        text.is_some_and(|text| self.insert(text))
    }

    pub fn insert(&mut self, text: String) -> bool {
        if text.len() > MAX_ITEM_BYTES
            || text.trim().is_empty()
            || self.items.first().is_some_and(|clip| clip.text == text)
        {
            return false;
        }
        let existing = self.items.iter().position(|clip| clip.text == text);
        let clip = if let Some(index) = existing {
            self.items.remove(index)
        } else {
            self.next_id += 1;
            Clip {
                id: self.next_id,
                text,
            }
        };
        self.items.insert(0, clip);
        self.items.truncate(CAPACITY);
        true
    }

    pub fn copied(&mut self, text: String) {
        self.last_seen = Some(text.clone());
        self.insert(text);
        self.baseline_pending = false;
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.last_seen = None;
        self.baseline_pending = true;
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.last_seen = None;
        self.baseline_pending = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_startup_clipboard_then_captures_new_text_exactly() {
        let mut history = History::new();
        assert!(!history.observe(Some("existing private text".into())));
        let text = "  سلام 👋\nsecond line\t ";
        assert!(history.observe(Some(text.into())));
        assert_eq!(history.items[0].text, text);
        assert!(!history.observe(Some(text.into())));
    }

    #[test]
    fn deduplicates_and_moves_existing_clip_to_front_without_changing_id() {
        let mut history = History::new();
        history.insert("one".into());
        let id = history.items[0].id;
        history.insert("two".into());
        history.insert("one".into());
        assert_eq!(history.items.len(), 2);
        assert_eq!(history.items[0].id, id);
        assert_eq!(history.items[0].text, "one");
    }

    #[test]
    fn bounds_count_and_utf8_bytes() {
        let mut history = History::new();
        for i in 0..100 {
            history.insert(i.to_string());
        }
        assert_eq!(history.items.len(), CAPACITY);
        assert_eq!(history.items[0].text, "99");
        assert_eq!(history.items[29].text, "70");
        assert!(!history.insert("é".repeat(MAX_ITEM_BYTES / 2 + 1)));
        assert!(history.insert("a".repeat(MAX_ITEM_BYTES)));
        assert!(!history.insert(" \t\n ".into()));
    }

    #[test]
    fn pause_and_resume_never_backfill_text_copied_while_paused() {
        let mut history = History::new();
        history.observe(None);
        history.observe(Some("keep".into()));
        history.toggle_pause();
        assert!(!history.observe(Some("paused secret".into())));
        history.toggle_pause();
        assert!(!history.observe(Some("paused secret".into())));
        assert!(!history.observe(Some("paused secret".into())));
        assert!(history.observe(Some("new after resume".into())));
        assert_eq!(history.items.len(), 2);
    }

    #[test]
    fn clear_does_not_recapture_the_current_clipboard() {
        let mut history = History::new();
        history.observe(None);
        history.observe(Some("clear me".into()));
        history.clear();
        assert!(!history.observe(Some("clear me".into())));
        assert!(!history.observe(Some("clear me".into())));
        assert!(history.items.is_empty());
    }

    #[test]
    fn copy_selection_does_not_create_duplicate_or_capture_feedback() {
        let mut history = History::new();
        history.insert("one".into());
        history.insert("two".into());
        history.copied("one".into());
        assert!(!history.observe(Some("one".into())));
        assert_eq!(history.items.len(), 2);
        assert_eq!(history.items[0].text, "one");
    }

    #[test]
    fn non_text_or_oversized_content_is_not_retained() {
        let mut history = History::new();
        history.observe(None);
        history.observe(Some("keep".into()));
        assert!(!history.observe(Some("x".repeat(MAX_ITEM_BYTES + 1))));
        assert!(history.last_seen.is_none());
        assert!(!history.observe(None));
        assert_eq!(history.items.len(), 1);
    }
}
