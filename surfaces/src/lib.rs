//! SHER-Display application surfaces (spec section 5-6).
//!
//! A `Surface` is the compositor's view of a client's drawable content —
//! independent of window semantics (title, decoration, layout), which the
//! `windows` crate owns. This crate is the seam between "an application
//! produced a buffer" and "the scene graph has something to composite":
//! it tracks buffer attachment, damage, clipping, and per-surface frame
//! callbacks, and is what a compatibility layer (Wayland, XWayland) drives.

use sher_common::{Error, ObjectId, Result};
use sher_display_scene::Rect;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceRole {
    Toplevel,
    Popup,
    Cursor,
    Subsurface,
}

#[derive(Clone, Debug)]
pub struct SurfaceState {
    pub id: ObjectId,
    pub client_id: ObjectId,
    pub role: SurfaceRole,
    pub buffer_id: Option<ObjectId>,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub opacity: f32,
    pub clip: Option<Rect>,
    pub visible: bool,
    pub damage: Vec<Rect>,
    /// Set once per commit; cleared after the compositor delivers the
    /// frame callback so the client knows it can draw the next frame.
    pub frame_callback_pending: bool,
}

impl SurfaceState {
    fn new(client_id: ObjectId, role: SurfaceRole) -> Self {
        SurfaceState {
            id: ObjectId::new(),
            client_id,
            role,
            buffer_id: None,
            width: 0,
            height: 0,
            scale: 1.0,
            opacity: 1.0,
            clip: None,
            visible: true,
            damage: Vec::new(),
            frame_callback_pending: false,
        }
    }
}

#[derive(Default)]
pub struct SurfaceManager {
    surfaces: HashMap<ObjectId, SurfaceState>,
}

impl SurfaceManager {
    pub fn new() -> Self {
        SurfaceManager {
            surfaces: HashMap::new(),
        }
    }

    pub fn create_surface(&mut self, client_id: ObjectId, role: SurfaceRole) -> ObjectId {
        let surface = SurfaceState::new(client_id, role);
        let id = surface.id;
        self.surfaces.insert(id, surface);
        id
    }

    pub fn destroy_surface(&mut self, id: &ObjectId) -> Result<()> {
        self.surfaces
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| Error::Device("surface not found".to_string()))
    }

    pub fn get(&self, id: &ObjectId) -> Option<&SurfaceState> {
        self.surfaces.get(id)
    }

    pub fn resize(&mut self, id: &ObjectId, width: u32, height: u32) -> Result<()> {
        let surface = self.require_mut(id)?;
        surface.width = width;
        surface.height = height;
        Ok(())
    }

    pub fn attach_buffer(&mut self, id: &ObjectId, buffer_id: ObjectId) -> Result<()> {
        self.require_mut(id)?.buffer_id = Some(buffer_id);
        Ok(())
    }

    pub fn damage(&mut self, id: &ObjectId, region: Rect) -> Result<()> {
        self.require_mut(id)?.damage.push(region);
        Ok(())
    }

    /// Commit clears accumulated damage and buffer attachment state,
    /// arming the frame callback — mirrors `wl_surface.commit` semantics
    /// without depending on any Wayland types directly, so XWayland or a
    /// native SHER client can drive the same state machine.
    pub fn commit(&mut self, id: &ObjectId) -> Result<Vec<Rect>> {
        let surface = self.require_mut(id)?;
        let damage = std::mem::take(&mut surface.damage);
        surface.frame_callback_pending = true;
        Ok(damage)
    }

    pub fn ack_frame_callback(&mut self, id: &ObjectId) -> Result<()> {
        self.require_mut(id)?.frame_callback_pending = false;
        Ok(())
    }

    pub fn set_opacity(&mut self, id: &ObjectId, opacity: f32) -> Result<()> {
        self.require_mut(id)?.opacity = opacity.clamp(0.0, 1.0);
        Ok(())
    }

    pub fn set_visible(&mut self, id: &ObjectId, visible: bool) -> Result<()> {
        self.require_mut(id)?.visible = visible;
        Ok(())
    }

    pub fn surfaces_for_client(&self, client_id: &ObjectId) -> Vec<&SurfaceState> {
        self.surfaces
            .values()
            .filter(|s| &s.client_id == client_id)
            .collect()
    }

    pub fn destroy_surfaces_for_client(&mut self, client_id: &ObjectId) {
        self.surfaces.retain(|_, s| &s.client_id != client_id);
    }

    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }

    fn require_mut(&mut self, id: &ObjectId) -> Result<&mut SurfaceState> {
        self.surfaces
            .get_mut(id)
            .ok_or_else(|| Error::Device("surface not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_drains_damage_and_arms_frame_callback() {
        let mut mgr = SurfaceManager::new();
        let client = ObjectId::new();
        let id = mgr.create_surface(client, SurfaceRole::Toplevel);

        mgr.damage(&id, Rect::new(0.0, 0.0, 100.0, 100.0)).unwrap();
        let drained = mgr.commit(&id).unwrap();

        assert_eq!(drained.len(), 1);
        assert!(mgr.get(&id).unwrap().damage.is_empty());
        assert!(mgr.get(&id).unwrap().frame_callback_pending);
    }

    #[test]
    fn destroying_client_drops_its_surfaces() {
        let mut mgr = SurfaceManager::new();
        let client = ObjectId::new();
        mgr.create_surface(client, SurfaceRole::Toplevel);
        mgr.create_surface(client, SurfaceRole::Popup);
        assert_eq!(mgr.len(), 2);

        mgr.destroy_surfaces_for_client(&client);
        assert!(mgr.is_empty());
    }

    #[test]
    fn operating_on_missing_surface_errors() {
        let mut mgr = SurfaceManager::new();
        let bogus = ObjectId::new();
        assert!(mgr.resize(&bogus, 10, 10).is_err());
    }
}
