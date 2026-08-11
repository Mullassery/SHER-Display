//! SHER-Display input routing (spec section 15-18, 21-24).
//!
//! Consumes `sher_input_core::InputService` — the real SHER-Input, not a
//! stand-in. `input_driver` (SHER-Kernel's raw evdev primitive) was a
//! documented temporary bridge until SHER-Input existed; it now does, so
//! this crate depends on it directly instead.
//!
//! The boundary stays exactly what `VISION.md` says it should be:
//! SHER-Input normalizes device activity into an ordered, sequenced,
//! modifier-annotated `InputEvent` stream and owns device lifecycle,
//! keyboard-layout mapping, and low-level capture enforcement
//! (`CaptureRegistry`). This crate does not re-derive any of that — it only
//! decides *where an event goes*: to a registered global shortcut, to the
//! surface holding keyboard focus, or to the surface under the pointer.
//! Isolation is enforced by construction, not by a permission check that
//! could be forgotten: `RoutedEvent::Focused` can only ever name the
//! currently tracked focus target, so there is no code path that hands one
//! surface another surface's input.
//!
//! `InputRouter` owns nothing SHER-Input itself owns twice over — it holds
//! the `Arc<InputService>` handed to it (SHER-Input's own top-level
//! orchestrator, analogous to SHER-Graphics's `GraphicsRuntime`) and its own
//! `broadcast::Receiver` subscription, never a second copy of device or
//! capture state.

pub use sher_input_core::{
    CaptureGuard, CaptureKind, InputDevice, InputDeviceId, InputEvent, InputEventPayload, InputService, KeyAction,
    KeyboardLayout, Modifiers, PhysicalKey, PointerButton, PointerEvent, PointerGrabMode,
};

use std::sync::Arc;
use tokio::sync::broadcast;

use sher_common::ObjectId;

/// Result of routing an event through focus + global-shortcut policy. There
/// is deliberately no variant that carries an arbitrary surface id chosen
/// by the caller — only the tracked focus/pointer-over target.
#[derive(Clone, Debug, PartialEq)]
pub enum RoutedEvent {
    Global(String),
    Focused(ObjectId, InputEvent),
    Dropped,
}

struct GlobalShortcut {
    modifiers: Modifiers,
    key: PhysicalKey,
    action: String,
}

pub struct InputRouter {
    service: Arc<InputService>,
    receiver: broadcast::Receiver<InputEvent>,
    keyboard_focus: Option<ObjectId>,
    pointer_over: Option<ObjectId>,
    shortcuts: Vec<GlobalShortcut>,
}

impl InputRouter {
    /// Subscribes to `service`'s canonical event stream. Does not spawn or
    /// own the service itself — whatever assembles the session constructs
    /// the one `Arc<InputService>` and hands it here, so nothing in
    /// SHER-Display can end up with a second, unsynchronized view of
    /// device/capture state (the same discipline `outputs` follows for
    /// SHER-Graphics's `GraphicsRuntime`).
    pub fn new(service: Arc<InputService>) -> Self {
        let receiver = service.subscribe();
        InputRouter { service, receiver, keyboard_focus: None, pointer_over: None, shortcuts: Vec::new() }
    }

    pub fn service(&self) -> &Arc<InputService> {
        &self.service
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
    pub fn register_global_shortcut(&mut self, modifiers: Modifiers, key: PhysicalKey, action: impl Into<String>) {
        self.shortcuts.push(GlobalShortcut { modifiers, key, action: action.into() });
    }

    /// Which layout SHER-Input maps physical keys through is a user
    /// setting SHER-Display owns; the mapping mechanism itself is
    /// SHER-Input's (spec section 17, 25). This is a pure pass-through.
    pub fn set_keyboard_layout(&self, layout: Box<dyn KeyboardLayout>) {
        self.service.set_layout(layout);
    }

    /// Section 16/23: exclusive input access is explicit, single-owner, and
    /// revocable — enforced by `CaptureRegistry`, not by this crate
    /// re-checking anything. Dropping the returned `CaptureGuard` releases
    /// it.
    pub fn request_pointer_capture(
        &self,
        owner: impl Into<String>,
        reason: impl Into<String>,
        mode: Option<PointerGrabMode>,
    ) -> sher_input_core::Result<CaptureGuard> {
        self.service.request_capture(CaptureKind::Pointer, owner, reason, mode)
    }

    /// Drains every event currently available on the canonical stream and
    /// routes each one. Call once per compositor frame — the same "once
    /// per frame" contract `InputService::flush_coalesced` documents, so
    /// coalesced motion/scroll never adds more than one frame of latency
    /// before a subscriber sees it.
    pub fn drain(&mut self) -> Vec<RoutedEvent> {
        let mut routed = Vec::new();
        loop {
            match self.receiver.try_recv() {
                Ok(event) => routed.push(self.route(event)),
                Err(broadcast::error::TryRecvError::Empty) | Err(broadcast::error::TryRecvError::Closed) => break,
                // A slow consumer fell behind SHER-Input's ring buffer. Skip
                // ahead rather than stalling the whole frame on history that
                // no longer matters for "what should the desktop do right now."
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
            }
        }
        routed
    }

    fn route(&self, event: InputEvent) -> RoutedEvent {
        if let InputEventPayload::Keyboard(ref key_event) = event.payload {
            if key_event.action == KeyAction::Down {
                if let Some(shortcut) =
                    self.shortcuts.iter().find(|s| s.modifiers == event.modifiers && s.key == key_event.physical_key)
                {
                    return RoutedEvent::Global(shortcut.action.clone());
                }
            }
        }

        let target = match event.payload {
            InputEventPayload::Keyboard(_) => self.keyboard_focus,
            InputEventPayload::Pointer(_) | InputEventPayload::Touch(_) | InputEventPayload::Tablet(_) => self.pointer_over,
            InputEventPayload::Gamepad(_) => self.keyboard_focus,
            InputEventPayload::DeviceAdded(_) | InputEventPayload::DeviceRemoved(_) => None,
        };

        match target {
            Some(surface_id) => RoutedEvent::Focused(surface_id, event),
            None => RoutedEvent::Dropped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sher_input_core::InputConfig;
    use sher_input_test::SimulatedController;

    fn router_with_controller() -> (InputRouter, SimulatedController) {
        let service = InputService::new(InputConfig::default());
        let router = InputRouter::new(Arc::clone(&service));
        let controller = SimulatedController::new(service.sink());
        (router, controller)
    }

    fn is_focused_delivery(event: &RoutedEvent) -> bool {
        matches!(event, RoutedEvent::Focused(_, _))
    }

    #[tokio::test]
    async fn unfocused_key_events_are_dropped_not_broadcast() {
        let (mut router, controller) = router_with_controller();
        let keyboard = controller.add_keyboard("test-kb");
        controller.tap_key(keyboard, PhysicalKey::KeyA);
        tokio::task::yield_now().await;

        let routed = router.drain();
        assert!(!routed.is_empty());
        assert!(!routed.iter().any(is_focused_delivery));
    }

    #[tokio::test]
    async fn focused_surface_receives_its_own_keys_only() {
        let (mut router, controller) = router_with_controller();
        let keyboard = controller.add_keyboard("test-kb");
        let surface = ObjectId::new();
        router.set_keyboard_focus(Some(surface));

        controller.press_key(keyboard, PhysicalKey::KeyA);
        tokio::task::yield_now().await;

        let routed = router.drain();
        assert!(routed.iter().any(|e| matches!(
            e,
            RoutedEvent::Focused(id, InputEvent { payload: InputEventPayload::Keyboard(kb), .. })
                if *id == surface && kb.physical_key == PhysicalKey::KeyA && kb.action == KeyAction::Down
        )));
    }

    #[tokio::test]
    async fn global_shortcut_intercepts_before_app_focus() {
        let (mut router, controller) = router_with_controller();
        let keyboard = controller.add_keyboard("test-kb");
        let surface = ObjectId::new();
        router.set_keyboard_focus(Some(surface));

        let mods = Modifiers { logo: true, ..Default::default() };
        router.register_global_shortcut(mods, PhysicalKey::ArrowRight, "workspace.switch.next");

        controller.press_key(keyboard, PhysicalKey::SuperLeft);
        controller.press_key(keyboard, PhysicalKey::ArrowRight);
        tokio::task::yield_now().await;

        let routed = router.drain();
        assert!(routed.contains(&RoutedEvent::Global("workspace.switch.next".to_string())));
    }

    #[tokio::test]
    async fn pointer_capture_is_exclusive_and_revocable() {
        let (router, _controller) = router_with_controller();

        let first = router.request_pointer_capture("compositor.drag", "window drag", None).unwrap();
        let second = router.request_pointer_capture("some.app", "unrelated", None);
        assert!(second.is_err());

        drop(first);
        let third = router.request_pointer_capture("some.app", "now allowed", None);
        assert!(third.is_ok());
    }

    #[tokio::test]
    async fn pointer_motion_routes_to_pointer_over_not_keyboard_focus() {
        let (mut router, controller) = router_with_controller();
        let mouse = controller.add_mouse("test-mouse");
        let keyboard_target = ObjectId::new();
        let pointer_target = ObjectId::new();
        router.set_keyboard_focus(Some(keyboard_target));
        router.set_pointer_over(Some(pointer_target));

        controller.move_relative(mouse, 5.0, 5.0);
        tokio::task::yield_now().await;
        router.service().flush_coalesced();

        let routed = router.drain();
        assert!(routed.iter().any(|e| matches!(e, RoutedEvent::Focused(id, _) if *id == pointer_target)));
        assert!(!routed.iter().any(|e| matches!(e, RoutedEvent::Focused(id, _) if *id == keyboard_target)));
    }
}
