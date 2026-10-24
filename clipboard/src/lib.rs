//! SHER-Display clipboard (spec section 20).
//!
//! Clipboard state is owned by one app at a time. Reads of another app's
//! clipboard (and of clipboard history) require a time-bound
//! `Permission::ClipboardAccess` grant from the SHER-Display security
//! boundary — the same capability model every other privileged operation
//! uses. History is bounded and keeps text formats only; binary payloads
//! (images, files) are offered to readers but never retained in history.

use std::collections::VecDeque;

use sher_common::{Error, Result};
use sher_display_security::{Permission, PermissionManager};

/// A single representation of the clipboard contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardContent {
    pub mime: String,
    pub data: Vec<u8>,
}

impl ClipboardContent {
    pub fn new(mime: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        ClipboardContent {
            mime: mime.into(),
            data: data.into(),
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::new("text/plain;charset=utf-8", text.into().into_bytes())
    }

    pub fn is_text(&self) -> bool {
        self.mime.starts_with("text/plain")
    }
}

/// The current clipboard offer, owned by exactly one app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardOffer {
    pub owner: String,
    pub sequence: u64,
    pub contents: Vec<ClipboardContent>,
}

impl ClipboardOffer {
    /// First representation matching `mime` exactly, else the first that
    /// starts with `mime` (so `text/plain` matches the charset-qualified
    /// text representation).
    pub fn format(&self, mime: &str) -> Option<&ClipboardContent> {
        self.contents
            .iter()
            .find(|content| content.mime == mime)
            .or_else(|| {
                self.contents
                    .iter()
                    .find(|content| content.mime.starts_with(mime))
            })
    }

    pub fn text(&self) -> Option<String> {
        self.format("text/plain")
            .and_then(|content| String::from_utf8(content.data.clone()).ok())
    }
}

/// One retained historical clipboard snapshot. Only text content is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardHistoryEntry {
    pub owner: String,
    pub sequence: u64,
    pub text: String,
    pub recorded_at: u64,
}

pub struct Clipboard {
    current: Option<ClipboardOffer>,
    history: VecDeque<ClipboardHistoryEntry>,
    history_limit: usize,
    next_sequence: u64,
}

impl Clipboard {
    pub fn new(history_limit: usize) -> Self {
        Clipboard {
            current: None,
            history: VecDeque::new(),
            history_limit,
            next_sequence: 0,
        }
    }

    pub fn current(&self) -> Option<&ClipboardOffer> {
        self.current.as_ref()
    }

    /// Replace the clipboard contents. The setting app becomes the owner.
    pub fn set_offer(
        &mut self,
        owner: impl Into<String>,
        contents: Vec<ClipboardContent>,
        now: u64,
    ) -> Result<ClipboardOffer> {
        if contents.is_empty() {
            return Err(Error::Device(
                "clipboard offer must contain at least one format".to_string(),
            ));
        }
        let owner = owner.into();
        self.next_sequence = self.next_sequence.saturating_add(1);
        let offer = ClipboardOffer {
            owner,
            sequence: self.next_sequence,
            contents,
        };
        self.current = Some(offer.clone());

        if self.history_limit > 0 {
            if let Some(text) = offer.text() {
                self.history.push_front(ClipboardHistoryEntry {
                    owner: offer.owner.clone(),
                    sequence: offer.sequence,
                    text,
                    recorded_at: now,
                });
                while self.history.len() > self.history_limit {
                    self.history.pop_back();
                }
            }
        }
        Ok(offer)
    }

    /// Read the current offer. The owning app can always read; every other
    /// app needs an active `ClipboardAccess` grant.
    pub fn read(
        &self,
        app_id: &str,
        permissions: &PermissionManager,
        now: u64,
    ) -> Result<ClipboardOffer> {
        let offer = self
            .current
            .as_ref()
            .ok_or_else(|| Error::Device("clipboard is empty".to_string()))?;
        if offer.owner == app_id || permissions.check(app_id, Permission::ClipboardAccess, now) {
            Ok(offer.clone())
        } else {
            Err(Error::Security(format!(
                "app {app_id:?} lacks ClipboardAccess to read clipboard owned by {:?}",
                offer.owner
            )))
        }
    }

    /// Read retained text history, newest first. Requires `ClipboardAccess`
    /// regardless of who owns the current offer.
    pub fn history(
        &self,
        app_id: &str,
        permissions: &PermissionManager,
        now: u64,
        limit: usize,
    ) -> Result<Vec<ClipboardHistoryEntry>> {
        if !permissions.check(app_id, Permission::ClipboardAccess, now) {
            return Err(Error::Security(format!(
                "app {app_id:?} lacks ClipboardAccess to read clipboard history"
            )));
        }
        Ok(self.history.iter().take(limit).cloned().collect())
    }

    /// Only the current owner may clear the clipboard.
    pub fn clear(&mut self, app_id: &str) -> Result<()> {
        match &self.current {
            Some(offer) if offer.owner == app_id => {
                self.current = None;
                Ok(())
            }
            Some(offer) => Err(Error::Security(format!(
                "app {app_id:?} does not own the clipboard (owner is {:?})",
                offer.owner
            ))),
            None => Ok(()),
        }
    }

    pub fn history_limit(&self) -> usize {
        self.history_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sher_display_security::PermissionTier;

    fn granted(app_id: &str) -> PermissionManager {
        let mut permissions = PermissionManager::new();
        permissions.grant(
            app_id,
            Permission::ClipboardAccess,
            PermissionTier::Low,
            1_000,
        );
        permissions
    }

    #[test]
    fn owner_can_read_without_a_grant() {
        let mut clipboard = Clipboard::new(4);
        clipboard
            .set_offer("app.owner", vec![ClipboardContent::text("hello")], 1_000)
            .unwrap();
        let offer = clipboard
            .read("app.owner", &PermissionManager::new(), 1_000)
            .unwrap();
        assert_eq!(offer.text().as_deref(), Some("hello"));
    }

    #[test]
    fn other_apps_need_clipboard_access() {
        let mut clipboard = Clipboard::new(4);
        clipboard
            .set_offer("app.owner", vec![ClipboardContent::text("secret")], 1_000)
            .unwrap();
        let denied = clipboard.read("app.other", &PermissionManager::new(), 1_000);
        assert!(matches!(denied, Err(Error::Security(_))));
        let allowed = clipboard
            .read("app.other", &granted("app.other"), 1_000)
            .unwrap();
        assert_eq!(allowed.text().as_deref(), Some("secret"));
    }

    #[test]
    fn expired_grant_is_denied() {
        let mut clipboard = Clipboard::new(4);
        clipboard
            .set_offer("app.owner", vec![ClipboardContent::text("secret")], 1_000)
            .unwrap();
        let permissions = granted("app.other");
        let expired = 1_000 + PermissionTier::Low.duration_secs();
        assert!(clipboard.read("app.other", &permissions, expired).is_err());
    }

    #[test]
    fn multiple_formats_and_prefix_matching() {
        let mut clipboard = Clipboard::new(4);
        clipboard
            .set_offer(
                "app.owner",
                vec![
                    ClipboardContent::text("hello"),
                    ClipboardContent::new("text/html", b"<b>hello</b>".to_vec()),
                ],
                1_000,
            )
            .unwrap();
        let offer = clipboard
            .read("app.owner", &PermissionManager::new(), 1_000)
            .unwrap();
        assert_eq!(offer.format("text/html").unwrap().data, b"<b>hello</b>");
        assert_eq!(offer.format("text/plain").unwrap().data, b"hello");
    }

    #[test]
    fn history_is_gated_and_bounded_newest_first() {
        let mut clipboard = Clipboard::new(2);
        for (index, text) in ["one", "two", "three"].iter().enumerate() {
            clipboard
                .set_offer(
                    "app.owner",
                    vec![ClipboardContent::text(*text)],
                    1_000 + index as u64,
                )
                .unwrap();
        }
        let permissions = granted("app.reader");
        let history = clipboard
            .history("app.reader", &permissions, 1_001, 10)
            .unwrap();
        assert_eq!(
            history
                .iter()
                .map(|entry| entry.text.as_str())
                .collect::<Vec<_>>(),
            vec!["three", "two"]
        );
        assert!(clipboard
            .history("app.reader", &PermissionManager::new(), 1_001, 10)
            .is_err());
    }

    #[test]
    fn binary_formats_are_offered_but_not_retained_in_history() {
        let mut clipboard = Clipboard::new(4);
        clipboard
            .set_offer(
                "app.owner",
                vec![ClipboardContent::new("image/png", vec![1, 2, 3])],
                1_000,
            )
            .unwrap();
        assert_eq!(clipboard.current().unwrap().contents.len(), 1);
        assert!(clipboard
            .history("app.owner", &granted("app.owner"), 1_000, 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn only_owner_can_clear() {
        let mut clipboard = Clipboard::new(4);
        clipboard
            .set_offer("app.owner", vec![ClipboardContent::text("hello")], 1_000)
            .unwrap();
        assert!(clipboard.clear("app.other").is_err());
        clipboard.clear("app.owner").unwrap();
        assert!(clipboard.current().is_none());
    }
}
