//! Wayland compatibility boundary (spec section 28, 32).
//!
//! ```text
//! Wayland Client  →  Wayland Compatibility Layer (this crate)  →  SHER-Display
//! ```
//!
//! Never the inverse: this crate translates Wayland-shaped protocol calls
//! (connect, create surface, attach buffer, commit, destroy) into calls
//! against SHER-Display's own state. It does not become SHER-Display's
//! native model.
//!
//! Two ownership rules keep this crate honest about the boundary:
//!
//! - It owns a `wayland_server::WaylandTransport` for client connections and
//!   buffer handles — SHER-Kernel's low-level primitive, not duplicated
//!   elsewhere (see the root `VISION.md`'s `WaylandCompositor` deprecation
//!   write-up).
//! - It does **not** own a `Compositor` or `WindowManager`. Every method
//!   that touches scene/surface/window state takes `&mut Compositor` /
//!   `&mut WindowManager` as a parameter instead of storing one, so there is
//!   exactly one owner of that state — whatever assembles the full session
//!   (ROADMAP.md Phase 3-5) — and this crate can't drift into the same
//!   duplicate-ownership mistake `outputs` originally made against
//!   SHER-Graphics.
//!
//! What this crate *does* own is purely protocol bookkeeping: which surface
//! belongs to which client, and which scene node / window a surface maps
//! to — needed so `disconnect_client` can guarantee no orphaned surfaces
//! remain (spec section 33), not general compositor state.

use sher_common::{Error, ObjectId, Result};
use sher_display_compositor::Compositor;
use sher_display_scene::{NodeKind, Rect, SceneNode};
use sher_display_surfaces::SurfaceRole;
use sher_display_windows::WindowManager;
use std::collections::HashMap;
use wayland_server::{WaylandClient, WaylandTransport};

#[derive(Default)]
struct ClientBookkeeping {
    surfaces: Vec<ObjectId>,
}

pub struct WaylandBridge {
    transport: WaylandTransport,
    clients: HashMap<ObjectId, ClientBookkeeping>,
    /// Surface id (in `sher_display_surfaces`) -> its scene node id.
    surface_nodes: HashMap<ObjectId, ObjectId>,
    /// Surface id -> the window wrapping it, if any (popups/cursors never
    /// get one; xdg-toplevel-equivalent surfaces do).
    surface_windows: HashMap<ObjectId, ObjectId>,
    /// Surface id -> owning client, so cleanup can find it from either
    /// direction.
    surface_clients: HashMap<ObjectId, ObjectId>,
}

impl WaylandBridge {
    pub fn new() -> Self {
        WaylandBridge {
            transport: WaylandTransport::new(),
            clients: HashMap::new(),
            surface_nodes: HashMap::new(),
            surface_windows: HashMap::new(),
            surface_clients: HashMap::new(),
        }
    }

    pub fn connect_client(&mut self, name: impl Into<String>) -> Result<ObjectId> {
        let client = WaylandClient {
            id: ObjectId::new(),
            name: name.into(),
            is_connected: false,
        };
        let id = client.id;
        self.transport.connect_client(client)?;
        self.clients.insert(id, ClientBookkeeping::default());
        Ok(id)
    }

    /// Tears down every surface (and window, if any) the client owns
    /// before dropping the connection — "no orphaned surfaces should
    /// remain" (spec section 33).
    pub fn disconnect_client(
        &mut self,
        compositor: &mut Compositor,
        windows: &mut WindowManager,
        client_id: &ObjectId,
    ) -> Result<()> {
        let bookkeeping = self
            .clients
            .remove(client_id)
            .ok_or_else(|| Error::Device("client not found".to_string()))?;
        for surface_id in bookkeeping.surfaces {
            self.teardown_surface(compositor, windows, &surface_id)?;
        }
        self.transport.disconnect_client(client_id)
    }

    /// Creates a SHER-Display surface plus its scene node for an existing
    /// client, mirroring `wl_compositor.create_surface`. `output_id` seeds
    /// which output the node composites onto; a real client wouldn't know
    /// this yet (it's chosen once the surface gets mapped), but the scene
    /// graph requires an output association to be composited at all, so
    /// this is the value `set_output` overwrites once placement is known.
    pub fn create_surface(
        &mut self,
        compositor: &mut Compositor,
        client_id: ObjectId,
        output_id: ObjectId,
        role: SurfaceRole,
    ) -> Result<ObjectId> {
        if !self.clients.contains_key(&client_id) {
            return Err(Error::Device("client not found".to_string()));
        }

        let surface_id = compositor.surfaces_mut().create_surface(client_id, role);

        let mut node = SceneNode::new(match role {
            SurfaceRole::Toplevel => NodeKind::Window,
            SurfaceRole::Popup => NodeKind::Popover,
            SurfaceRole::Cursor => NodeKind::Cursor,
            SurfaceRole::Subsurface => NodeKind::Window,
        });
        node.output_id = Some(output_id);
        let node_id = compositor.scene_mut().insert(node);

        self.surface_nodes.insert(surface_id, node_id);
        self.surface_clients.insert(surface_id, client_id);
        self.clients
            .get_mut(&client_id)
            .unwrap()
            .surfaces
            .push(surface_id);

        Ok(surface_id)
    }

    /// Wraps a `Toplevel` surface with window semantics — the
    /// xdg-toplevel-equivalent step. Section 28 calls out "XDG shell where
    /// appropriate"; this is that seam, without committing to XDG's wire
    /// format.
    pub fn create_toplevel_window(
        &mut self,
        windows: &mut WindowManager,
        surface_id: ObjectId,
        app_id: impl Into<String>,
        title: impl Into<String>,
    ) -> Result<ObjectId> {
        if !self.surface_nodes.contains_key(&surface_id) {
            return Err(Error::Device("surface not found".to_string()));
        }
        let window_id = windows.create_window(surface_id, app_id, title);
        self.surface_windows.insert(surface_id, window_id);
        Ok(window_id)
    }

    /// `wl_surface.attach` + resize in one step: allocates a buffer via the
    /// kernel transport and attaches it to the SHER-Display surface.
    pub fn attach_buffer(
        &mut self,
        compositor: &mut Compositor,
        surface_id: &ObjectId,
        width: u32,
        height: u32,
        format: u32,
    ) -> Result<ObjectId> {
        let buffer = self.transport.create_buffer(width, height, format)?;
        compositor
            .surfaces_mut()
            .resize(surface_id, width, height)?;
        compositor
            .surfaces_mut()
            .attach_buffer(surface_id, buffer.id)?;
        Ok(buffer.id)
    }

    pub fn damage(
        &mut self,
        compositor: &mut Compositor,
        surface_id: &ObjectId,
        region: Rect,
    ) -> Result<()> {
        compositor.surfaces_mut().damage(surface_id, region)
    }

    /// `wl_surface.commit`: drains the surface's damage into its scene
    /// node so the next `Compositor::tick` picks it up.
    pub fn commit(&mut self, compositor: &mut Compositor, surface_id: &ObjectId) -> Result<()> {
        let node_id = *self
            .surface_nodes
            .get(surface_id)
            .ok_or_else(|| Error::Device("surface not found".to_string()))?;
        compositor.commit_surface(surface_id, &node_id)
    }

    /// `wl_surface.destroy` for a single surface, without touching the
    /// client connection — used directly by clients and internally by
    /// `disconnect_client`.
    pub fn destroy_surface(
        &mut self,
        compositor: &mut Compositor,
        windows: &mut WindowManager,
        surface_id: &ObjectId,
    ) -> Result<()> {
        self.teardown_surface(compositor, windows, surface_id)?;
        if let Some(client_id) = self.surface_clients.get(surface_id) {
            if let Some(bookkeeping) = self.clients.get_mut(client_id) {
                bookkeeping.surfaces.retain(|s| s != surface_id);
            }
        }
        Ok(())
    }

    fn teardown_surface(
        &mut self,
        compositor: &mut Compositor,
        windows: &mut WindowManager,
        surface_id: &ObjectId,
    ) -> Result<()> {
        if let Some(window_id) = self.surface_windows.remove(surface_id) {
            windows.destroy_window(&window_id)?;
        }
        if let Some(node_id) = self.surface_nodes.remove(surface_id) {
            compositor.scene_mut().remove(&node_id)?;
        }
        self.surface_clients.remove(surface_id);
        compositor.surfaces_mut().destroy_surface(surface_id)
    }

    pub fn node_for_surface(&self, surface_id: &ObjectId) -> Option<ObjectId> {
        self.surface_nodes.get(surface_id).copied()
    }

    pub fn window_for_surface(&self, surface_id: &ObjectId) -> Option<ObjectId> {
        self.surface_windows.get(surface_id).copied()
    }
}

impl Default for WaylandBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_surface_lifecycle_produces_a_frame() {
        let mut bridge = WaylandBridge::new();
        let mut compositor = Compositor::new();
        let mut windows = WindowManager::new();

        let output = ObjectId::new();
        compositor.register_output(output, 60);
        compositor.tick(std::time::Duration::from_millis(17)); // consume forced first redraw

        let client = bridge.connect_client("weston-terminal").unwrap();
        let surface = bridge
            .create_surface(&mut compositor, client, output, SurfaceRole::Toplevel)
            .unwrap();
        bridge
            .create_toplevel_window(&mut windows, surface, "org.sher.Terminal", "Terminal")
            .unwrap();

        bridge
            .attach_buffer(&mut compositor, &surface, 800, 600, 0x34325241)
            .unwrap();
        bridge
            .damage(&mut compositor, &surface, Rect::new(0.0, 0.0, 800.0, 600.0))
            .unwrap();
        bridge.commit(&mut compositor, &surface).unwrap();

        let reports = compositor.tick(std::time::Duration::from_millis(17));
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].damage.len(), 1);
    }

    #[test]
    fn disconnecting_client_leaves_no_orphaned_surfaces() {
        let mut bridge = WaylandBridge::new();
        let mut compositor = Compositor::new();
        let mut windows = WindowManager::new();
        let output = ObjectId::new();

        let client = bridge.connect_client("app").unwrap();
        let s1 = bridge
            .create_surface(&mut compositor, client, output, SurfaceRole::Toplevel)
            .unwrap();
        let s2 = bridge
            .create_surface(&mut compositor, client, output, SurfaceRole::Popup)
            .unwrap();
        bridge
            .create_toplevel_window(&mut windows, s1, "app", "Window")
            .unwrap();

        assert_eq!(compositor.surfaces().len(), 2);

        bridge
            .disconnect_client(&mut compositor, &mut windows, &client)
            .unwrap();

        assert_eq!(compositor.surfaces().len(), 0);
        assert!(compositor.scene().is_empty());
        assert!(bridge.node_for_surface(&s1).is_none());
        assert!(bridge.node_for_surface(&s2).is_none());
        assert!(bridge.window_for_surface(&s1).is_none());
    }

    #[test]
    fn destroying_one_surface_does_not_touch_a_sibling() {
        let mut bridge = WaylandBridge::new();
        let mut compositor = Compositor::new();
        let mut windows = WindowManager::new();
        let output = ObjectId::new();

        let client = bridge.connect_client("app").unwrap();
        let s1 = bridge
            .create_surface(&mut compositor, client, output, SurfaceRole::Toplevel)
            .unwrap();
        let s2 = bridge
            .create_surface(&mut compositor, client, output, SurfaceRole::Toplevel)
            .unwrap();

        bridge
            .destroy_surface(&mut compositor, &mut windows, &s1)
            .unwrap();

        assert!(compositor.surfaces().get(&s1).is_none());
        assert!(compositor.surfaces().get(&s2).is_some());
    }

    #[test]
    fn creating_surface_for_unknown_client_fails() {
        let mut bridge = WaylandBridge::new();
        let mut compositor = Compositor::new();
        let bogus_client = ObjectId::new();

        let result = bridge.create_surface(
            &mut compositor,
            bogus_client,
            ObjectId::new(),
            SurfaceRole::Toplevel,
        );
        assert!(result.is_err());
    }
}
