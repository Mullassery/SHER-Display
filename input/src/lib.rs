//! SHER-Display input routing (spec section 15-18).
//!
//! Wraps `input_driver`'s raw device/event primitives (SHER-Kernel: "what
//! happened on this device") with routing, focus, and isolation policy
//! (SHER-Display: "who is allowed to see it"). Section 16 is the load-
//! bearing constraint here: a surface only ever receives an event if it is
//! the current focus target — there is no API that hands an application
//! another application's input, by construction rather than by a
//! permission check that could be forgotten.

pub use input_driver::{InputDevice, InputDeviceType, KeyEvent, KeyEventType, MotionEvent, Touch};

use input_driver::InputDriver;
use sher_common::ObjectId;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
}

#[derive(Clone, Debug)]
pub struct GlobalShortcut {
    pub id: u64,
    pub modifiers: KeyModifiers,
    pub keycode: u32,
    pub action: String,
}

/// Result of routing a key event through focus + global-shortcut policy.
/// There is deliberately no variant that carries an arbitrary surface id
/// chosen by the caller — only the tracked focus target.
#[derive(Clone, Debug)]
pub enum RoutedKeyEvent {
    Global(String),
    Application(ObjectId, KeyEvent),
    Dropped,
}

// Manual impl: `input_driver::KeyEvent` (SHER-Kernel) doesn't derive
// `PartialEq`, and it isn't this crate's type to add that to.
impl PartialEq for RoutedKeyEvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RoutedKeyEvent::Global(a), RoutedKeyEvent::Global(b)) => a == b,
            (RoutedKeyEvent::Application(a_id, a_ev), RoutedKeyEvent::Application(b_id, b_ev)) => {
                a_id == b_id && a_ev.keycode == b_ev.keycode && a_ev.event_type == b_ev.event_type && a_ev.timestamp == b_ev.timestamp
            }
            (RoutedKeyEvent::Dropped, RoutedKeyEvent::Dropped) => true,
            _ => false,
        }
    }
}

pub struct InputRouter {
    driver: InputDriver,
    keyboard_focus: Option<ObjectId>,
    pointer_over: Option<ObjectId>,
    pointer_position: (i32, i32),
    shortcuts: HashMap<(KeyModifiers, u32), GlobalShortcut>,
    next_shortcut_id: u64,
    active_layout: String,
    installed_layouts: Vec<String>,
}

impl InputRouter {
    pub fn new() -> Self {
        InputRouter {
            driver: InputDriver::new(),
            keyboard_focus: None,
            pointer_over: None,
            pointer_position: (0, 0),
            shortcuts: HashMap::new(),
            next_shortcut_id: 1,
            active_layout: "us".to_string(),
            installed_layouts: vec!["us".to_string()],
        }
    }

    pub fn devices(&mut self) -> &mut InputDriver {
        &mut self.driver
    }

    pub fn set_keyboard_focus(&mut self, surface_id: Option<ObjectId>) {
        self.keyboard_focus = surface_id;
    }

    pub fn keyboard_focus(&self) -> Option<ObjectId> {
        self.keyboard_focus
    }

    pub fn set_pointer_over(&mut self, surface_id: Option<ObjectId>) {
        self.pointer_over = surface_id;
    }

    pub fn pointer_over(&self) -> Option<ObjectId> {
        self.pointer_over
    }

    /// Registered by SHER-Display itself (settings, session, desktop) —
    /// never by an arbitrary application (section 17).
    pub fn register_global_shortcut(&mut self, modifiers: KeyModifiers, keycode: u32, action: impl Into<String>) -> u64 {
        let id = self.next_shortcut_id;
        self.next_shortcut_id += 1;
        self.shortcuts.insert((modifiers, keycode), GlobalShortcut { id, modifiers, keycode, action: action.into() });
        id
    }

    pub fn unregister_global_shortcut(&mut self, id: u64) {
        self.shortcuts.retain(|_, s| s.id != id);
    }

    pub fn dispatch_key(&mut self, modifiers: KeyModifiers, event: KeyEvent) -> RoutedKeyEvent {
        if let Some(shortcut) = self.shortcuts.get(&(modifiers, event.keycode)) {
            return RoutedKeyEvent::Global(shortcut.action.clone());
        }
        match self.keyboard_focus {
            Some(surface_id) => RoutedKeyEvent::Application(surface_id, event),
            None => RoutedKeyEvent::Dropped,
        }
    }

    /// Updates tracked pointer position and returns which surface (if any)
    /// should receive the motion — the surface currently under the
    /// pointer, set via `set_pointer_over` by the compositor's hit test.
    pub fn dispatch_motion(&mut self, event: MotionEvent) -> Option<ObjectId> {
        self.pointer_position = (event.x, event.y);
        self.pointer_over
    }

    pub fn pointer_position(&self) -> (i32, i32) {
        self.pointer_position
    }

    pub fn install_layout(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !self.installed_layouts.contains(&name) {
            self.installed_layouts.push(name);
        }
    }

    pub fn set_active_layout(&mut self, name: impl Into<String>) -> bool {
        let name = name.into();
        if self.installed_layouts.contains(&name) {
            self.active_layout = name;
            true
        } else {
            false
        }
    }

    pub fn active_layout(&self) -> &str {
        &self.active_layout
    }
}

impl Default for InputRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: u32) -> KeyEvent {
        KeyEvent { keycode: code, event_type: KeyEventType::Press, timestamp: 0 }
    }

    #[test]
    fn unfocused_key_events_are_dropped_not_broadcast() {
        let mut router = InputRouter::new();
        let routed = router.dispatch_key(KeyModifiers::default(), key(30));
        assert_eq!(routed, RoutedKeyEvent::Dropped);
    }

    #[test]
    fn focused_surface_receives_its_own_keys_only() {
        let mut router = InputRouter::new();
        let a = ObjectId::new();
        router.set_keyboard_focus(Some(a));

        let routed = router.dispatch_key(KeyModifiers::default(), key(30));
        assert_eq!(routed, RoutedKeyEvent::Application(a, key(30)));
    }

    #[test]
    fn global_shortcut_intercepts_before_app_focus() {
        let mut router = InputRouter::new();
        let a = ObjectId::new();
        router.set_keyboard_focus(Some(a));

        let mods = KeyModifiers { logo: true, ..Default::default() };
        router.register_global_shortcut(mods, 20, "workspace.switch.next");

        let routed = router.dispatch_key(mods, key(20));
        assert_eq!(routed, RoutedKeyEvent::Global("workspace.switch.next".to_string()));
    }

    #[test]
    fn switching_layout_requires_prior_install() {
        let mut router = InputRouter::new();
        assert!(!router.set_active_layout("de"));
        router.install_layout("de");
        assert!(router.set_active_layout("de"));
        assert_eq!(router.active_layout(), "de");
    }
}
