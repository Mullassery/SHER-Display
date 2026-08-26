//! XWayland compatibility boundary (spec section 29).
//!
//! ```text
//! X11 Application → XWayland Compatibility (this crate) → Wayland/SHER Surface → SHER-Display
//! ```
//!
//! Scope is deliberately narrow. A real XWayland is an external, unmodified
//! X server process that itself connects to the compositor as an ordinary
//! Wayland client — it does not need SHER-Display to speak X11 wire
//! protocol at all. What SHER-Display needs is the *coordinate mapping*
//! between an X11 window (identified by its 32-bit XID) and the Wayland
//! surface XWayland created to back it, so window-management operations
//! requested in X11 terms (raise this XID, move this XID) can be resolved
//! to a `sher_display_surfaces` surface id.
//!
//! This crate is exactly that mapping table. It intentionally does not
//! depend on `sher_display_compat_wayland`, `sher_display_compositor`, or
//! `sher_display_windows` — spawning the XWayland process and driving it as
//! a Wayland client through `WaylandBridge` is the caller's job (X11 is not
//! the native architecture; see section 29's "do NOT make X11 the native
//! display architecture"). `map_window` only records a mapping that must
//! already be backed by a real surface created elsewhere.

use sher_common::{Error, ObjectId, Result};
use std::collections::HashMap;

/// X11 XIDs are 32-bit.
pub type X11WindowId = u32;

#[derive(Default)]
pub struct XWaylandBridge {
    window_to_surface: HashMap<X11WindowId, ObjectId>,
    surface_to_window: HashMap<ObjectId, X11WindowId>,
}

impl XWaylandBridge {
    pub fn new() -> Self {
        XWaylandBridge {
            window_to_surface: HashMap::new(),
            surface_to_window: HashMap::new(),
        }
    }

    /// Records that X11 window `xid` is backed by `surface_id`. The
    /// surface must already exist (created through
    /// `sher_display_compat_wayland::WaylandBridge`, since XWayland itself
    /// is a Wayland client) — this call does not create anything.
    pub fn map_window(&mut self, xid: X11WindowId, surface_id: ObjectId) -> Result<()> {
        if self.window_to_surface.contains_key(&xid) {
            return Err(Error::Device(format!("X11 window {xid} is already mapped")));
        }
        self.window_to_surface.insert(xid, surface_id);
        self.surface_to_window.insert(surface_id, xid);
        Ok(())
    }

    pub fn unmap_window(&mut self, xid: X11WindowId) -> Result<()> {
        let surface_id = self
            .window_to_surface
            .remove(&xid)
            .ok_or_else(|| Error::Device(format!("X11 window {xid} is not mapped")))?;
        self.surface_to_window.remove(&surface_id);
        Ok(())
    }

    pub fn surface_for_window(&self, xid: X11WindowId) -> Option<ObjectId> {
        self.window_to_surface.get(&xid).copied()
    }

    pub fn window_for_surface(&self, surface_id: &ObjectId) -> Option<X11WindowId> {
        self.surface_to_window.get(surface_id).copied()
    }

    pub fn len(&self) -> usize {
        self.window_to_surface.len()
    }

    pub fn is_empty(&self) -> bool {
        self.window_to_surface.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_is_bidirectional() {
        let mut bridge = XWaylandBridge::new();
        let surface = ObjectId::new();
        bridge.map_window(42, surface).unwrap();

        assert_eq!(bridge.surface_for_window(42), Some(surface));
        assert_eq!(bridge.window_for_surface(&surface), Some(42));
    }

    #[test]
    fn cannot_double_map_the_same_xid() {
        let mut bridge = XWaylandBridge::new();
        bridge.map_window(1, ObjectId::new()).unwrap();
        assert!(bridge.map_window(1, ObjectId::new()).is_err());
    }

    #[test]
    fn unmapping_clears_both_directions() {
        let mut bridge = XWaylandBridge::new();
        let surface = ObjectId::new();
        bridge.map_window(7, surface).unwrap();

        bridge.unmap_window(7).unwrap();

        assert!(bridge.surface_for_window(7).is_none());
        assert!(bridge.window_for_surface(&surface).is_none());
        assert!(bridge.is_empty());
    }

    #[test]
    fn unmapping_unknown_window_errors() {
        let mut bridge = XWaylandBridge::new();
        assert!(bridge.unmap_window(999).is_err());
    }
}
