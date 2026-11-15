use crate::content::{Content, Kind, MAX_HISTORY};

pub const CAPACITY: usize = 30;
#[cfg(test)]
pub const MAX_ITEM_BYTES: usize = crate::content::MAX_TEXT;

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub id: u32,
    pub text: String,
    pub kind: Kind,
    pub preview: Option<Vec<u8>>,
    pub content: Content,
}

#[derive(Default)]
pub struct History {
    pub items: Vec<Clip>,
    pub paused: bool,
    last_seen: Option<u64>,
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
    pub fn observe(&mut self, text: Option<Content>) -> bool {
        if self.paused {
            return false;
        }
        let text = text.filter(Content::valid);
        let fingerprint = text.as_ref().map(Content::fingerprint);
        let changed = fingerprint != self.last_seen;
        self.last_seen = fingerprint;
        if self.baseline_pending {
            self.baseline_pending = false;
            return false;
        }
        if !changed {
            return false;
        }
        text.is_some_and(|text| self.insert(text))
    }

    pub fn insert(&mut self, content: Content) -> bool {
        if !content.valid()
            || self
                .items
                .first()
                .is_some_and(|clip| clip.content == content)
        {
            return false;
        }
        let existing = self.items.iter().position(|clip| clip.content == content);
        let clip = if let Some(index) = existing {
            self.items.remove(index)
        } else {
            self.next_id += 1;
            Clip {
                id: self.next_id,
                text: content.label(),
                kind: content.kind(),
                preview: content.preview(),
                content,
            }
        };
        self.items.insert(0, clip);
        self.items.truncate(CAPACITY);
        while self.items.iter().map(|c| c.content.size()).sum::<usize>() > MAX_HISTORY {
            self.items.pop();
        }
        true
    }

    pub fn copied(&mut self, text: Content) {
        self.last_seen = Some(text.fingerprint());
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
            history.insert(i.to_string().into());
        }
        assert_eq!(history.items.len(), CAPACITY);
        assert_eq!(history.items[0].text, "99");
        assert_eq!(history.items[29].text, "70");
        assert!(!history.insert("é".repeat(MAX_ITEM_BYTES / 2 + 1).into()));
        assert!(history.insert("a".repeat(MAX_ITEM_BYTES).into()));
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
        assert!(!history.observe(Some("x".repeat(MAX_ITEM_BYTES + 1).into())));
        assert!(history.last_seen.is_none());
        assert!(!history.observe(None));
        assert_eq!(history.items.len(), 1);
    }

    #[test]
    fn images_share_payloads_deduplicate_and_evict_by_total_size() {
        use crate::content::MAX_IMAGE;
        let mut history = History::new();
        for n in 1..=4 {
            history.insert(Content::Image {
                mime: "image/png",
                bytes: vec![n; MAX_IMAGE].into(),
            });
        }
        assert_eq!(history.items.len(), 3);
        let content = history.items[2].content.clone();
        let id = history.items[2].id;
        history.copied(content.clone());
        assert_eq!(history.items[0].id, id);
        assert_eq!(history.items.len(), 3);
        assert!(!history.observe(Some(content)));
        assert!(!history.insert(Content::Image {
            mime: "image/png",
            bytes: vec![0; MAX_IMAGE + 1].into()
        }));
    }
}
