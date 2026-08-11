//! SHER-Display window management (spec section 8-9).
//!
//! Deliberately separate from `surfaces`: a window wraps a surface with
//! desktop-level semantics (title, layout, activation, modality) that an
//! application never sees and a compatibility layer never needs. Keeping
//! this boundary means a window can be re-parented to a different surface
//! (e.g. XWayland re-mapping) without the compositor's scene/composition
//! state caring.

use sher_common::{Error, ObjectId, Result};
use sher_display_scene::{Point, Size};
use std::collections::HashMap;

/// Minimum layout set required by spec section 9. Additional modes
/// (split-screen, grid, picture-in-picture) are expected to land as new
/// variants once a layout-policy trait exists — not blocking Phase 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    Floating,
    Tiled,
    Maximized,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Debug, Default)]
pub struct WindowRules {
    pub force_floating: bool,
    pub always_on_top: bool,
    pub skip_taskbar: bool,
}

#[derive(Clone, Debug)]
pub struct WindowState {
    pub id: ObjectId,
    pub surface_id: ObjectId,
    pub app_id: String,
    pub title: String,
    pub layout: LayoutMode,
    pub position: Point,
    pub size: Size,
    pub minimized: bool,
    pub active: bool,
    pub modal: bool,
    pub transient_for: Option<ObjectId>,
    pub always_on_top: bool,
    pub rules: WindowRules,
}

impl WindowState {
    fn new(surface_id: ObjectId, app_id: String, title: String) -> Self {
        WindowState {
            id: ObjectId::new(),
            surface_id,
            app_id,
            title,
            layout: LayoutMode::Floating,
            position: Point::default(),
            size: Size::default(),
            minimized: false,
            active: false,
            modal: false,
            transient_for: None,
            always_on_top: false,
            rules: WindowRules::default(),
        }
    }
}

#[derive(Default)]
pub struct WindowManager {
    windows: HashMap<ObjectId, WindowState>,
    active_window: Option<ObjectId>,
}

impl WindowManager {
    pub fn new() -> Self {
        WindowManager { windows: HashMap::new(), active_window: None }
    }

    pub fn create_window(&mut self, surface_id: ObjectId, app_id: impl Into<String>, title: impl Into<String>) -> ObjectId {
        let window = WindowState::new(surface_id, app_id.into(), title.into());
        let id = window.id;
        self.windows.insert(id, window);
        id
    }

    pub fn destroy_window(&mut self, id: &ObjectId) -> Result<()> {
        self.windows.remove(id).ok_or_else(|| Error::Device("window not found".to_string()))?;
        if self.active_window.as_ref() == Some(id) {
            self.active_window = None;
        }
        Ok(())
    }

    pub fn get(&self, id: &ObjectId) -> Option<&WindowState> {
        self.windows.get(id)
    }

    pub fn activate(&mut self, id: &ObjectId) -> Result<()> {
        if !self.windows.contains_key(id) {
            return Err(Error::Device("window not found".to_string()));
        }
        if let Some(prev) = self.active_window.take() {
            if let Some(w) = self.windows.get_mut(&prev) {
                w.active = false;
            }
        }
        self.windows.get_mut(id).unwrap().active = true;
        self.active_window = Some(*id);
        Ok(())
    }

    pub fn active_window(&self) -> Option<ObjectId> {
        self.active_window
    }

    pub fn minimize(&mut self, id: &ObjectId) -> Result<()> {
        self.require_mut(id)?.minimized = true;
        Ok(())
    }

    pub fn restore(&mut self, id: &ObjectId) -> Result<()> {
        self.require_mut(id)?.minimized = false;
        Ok(())
    }

    pub fn set_layout(&mut self, id: &ObjectId, layout: LayoutMode) -> Result<()> {
        self.require_mut(id)?.layout = layout;
        Ok(())
    }

    pub fn move_to(&mut self, id: &ObjectId, position: Point) -> Result<()> {
        let w = self.require_mut(id)?;
        if w.layout == LayoutMode::Fullscreen || w.layout == LayoutMode::Maximized {
            return Err(Error::Device("cannot move a fullscreen or maximized window".to_string()));
        }
        w.position = position;
        Ok(())
    }

    pub fn resize_to(&mut self, id: &ObjectId, size: Size) -> Result<()> {
        self.require_mut(id)?.size = size;
        Ok(())
    }

    pub fn snap(&mut self, id: &ObjectId, edge: SnapEdge, output_size: Size) -> Result<()> {
        let half = Size { width: output_size.width / 2.0, height: output_size.height };
        let w = self.require_mut(id)?;
        w.layout = LayoutMode::Tiled;
        w.size = half;
        w.position = match edge {
            SnapEdge::Left => Point { x: 0.0, y: 0.0 },
            SnapEdge::Right => Point { x: half.width, y: 0.0 },
            SnapEdge::Top | SnapEdge::Bottom => Point { x: 0.0, y: 0.0 },
        };
        Ok(())
    }

    pub fn set_modal(&mut self, id: &ObjectId, transient_for: ObjectId) -> Result<()> {
        let w = self.require_mut(id)?;
        w.modal = true;
        w.transient_for = Some(transient_for);
        Ok(())
    }

    pub fn windows_for_app(&self, app_id: &str) -> Vec<&WindowState> {
        self.windows.values().filter(|w| w.app_id == app_id).collect()
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    fn require_mut(&mut self, id: &ObjectId) -> Result<&mut WindowState> {
        self.windows.get_mut(id).ok_or_else(|| Error::Device("window not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr_with_window() -> (WindowManager, ObjectId) {
        let mut mgr = WindowManager::new();
        let id = mgr.create_window(ObjectId::new(), "app.example", "Untitled");
        (mgr, id)
    }

    #[test]
    fn activation_deactivates_previous_window() {
        let mut mgr = WindowManager::new();
        let a = mgr.create_window(ObjectId::new(), "a", "A");
        let b = mgr.create_window(ObjectId::new(), "b", "B");

        mgr.activate(&a).unwrap();
        assert!(mgr.get(&a).unwrap().active);

        mgr.activate(&b).unwrap();
        assert!(!mgr.get(&a).unwrap().active);
        assert!(mgr.get(&b).unwrap().active);
        assert_eq!(mgr.active_window(), Some(b));
    }

    #[test]
    fn fullscreen_window_rejects_move() {
        let (mut mgr, id) = mgr_with_window();
        mgr.set_layout(&id, LayoutMode::Fullscreen).unwrap();
        assert!(mgr.move_to(&id, Point { x: 10.0, y: 10.0 }).is_err());
    }

    #[test]
    fn snap_sets_tiled_layout_and_half_width() {
        let (mut mgr, id) = mgr_with_window();
        mgr.snap(&id, SnapEdge::Left, Size { width: 1920.0, height: 1080.0 }).unwrap();
        let w = mgr.get(&id).unwrap();
        assert_eq!(w.layout, LayoutMode::Tiled);
        assert_eq!(w.size.width, 960.0);
    }

    #[test]
    fn destroying_active_window_clears_active_slot() {
        let (mut mgr, id) = mgr_with_window();
        mgr.activate(&id).unwrap();
        mgr.destroy_window(&id).unwrap();
        assert_eq!(mgr.active_window(), None);
    }
}
