//! SHER-Display cursor system (spec section 19).
//!
//! Tracks cursor shape/theme/position state and picks hardware vs software
//! rendering. Whether a GPU plane is actually available for a hardware
//! cursor is a `gpu_driver`/`gpu_abstraction` fact this crate doesn't
//! know yet — `prefers_hardware()` records intent so the compositor can
//! decide once that capability is queryable.

use sher_common::ObjectId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorShape {
    Default,
    Text,
    Pointer,
    Grab,
    Grabbing,
    ResizeHorizontal,
    ResizeVertical,
    Wait,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    Hardware,
    Software,
}

#[derive(Clone, Debug)]
pub struct CursorState {
    pub shape: CursorShape,
    pub theme: String,
    pub size_px: u32,
    pub position: (i32, i32),
    pub visible: bool,
    pub render_mode: RenderMode,
    /// Set for `CursorShape::Custom`, e.g. an application-supplied cursor
    /// surface (drag icon, app-defined pointer).
    pub custom_surface: Option<ObjectId>,
}

impl Default for CursorState {
    fn default() -> Self {
        CursorState {
            shape: CursorShape::Default,
            theme: "sher-default".to_string(),
            size_px: 24,
            position: (0, 0),
            visible: true,
            render_mode: RenderMode::Hardware,
            custom_surface: None,
        }
    }
}

pub struct CursorManager {
    state: CursorState,
    /// Accessibility floor: `set_size` below this is clamped, per section
    /// 27's large-cursor requirement.
    min_accessible_size_px: u32,
}

impl CursorManager {
    pub fn new() -> Self {
        CursorManager {
            state: CursorState::default(),
            min_accessible_size_px: 16,
        }
    }

    pub fn state(&self) -> &CursorState {
        &self.state
    }

    pub fn set_shape(&mut self, shape: CursorShape) {
        if shape != CursorShape::Custom {
            self.state.custom_surface = None;
        }
        self.state.shape = shape;
    }

    pub fn set_custom(&mut self, surface_id: ObjectId) {
        self.state.shape = CursorShape::Custom;
        self.state.custom_surface = Some(surface_id);
    }

    pub fn move_to(&mut self, position: (i32, i32)) {
        self.state.position = position;
    }

    pub fn set_theme(&mut self, theme: impl Into<String>) {
        self.state.theme = theme.into();
    }

    pub fn set_size(&mut self, size_px: u32) {
        self.state.size_px = size_px.max(self.min_accessible_size_px);
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.state.visible = visible;
    }

    /// Compositor calls this once GPU cursor-plane support is known; falls
    /// back to software compositing (drawing the cursor into the scene
    /// graph like any other node) when hardware isn't available.
    pub fn negotiate_render_mode(&mut self, hardware_available: bool) {
        self.state.render_mode = if hardware_available {
            RenderMode::Hardware
        } else {
            RenderMode::Software
        };
    }
}

impl Default for CursorManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessibility_floor_clamps_small_sizes() {
        let mut mgr = CursorManager::new();
        mgr.set_size(4);
        assert_eq!(mgr.state().size_px, 16);
    }

    #[test]
    fn setting_non_custom_shape_clears_custom_surface() {
        let mut mgr = CursorManager::new();
        mgr.set_custom(ObjectId::new());
        assert!(mgr.state().custom_surface.is_some());

        mgr.set_shape(CursorShape::Pointer);
        assert!(mgr.state().custom_surface.is_none());
    }

    #[test]
    fn falls_back_to_software_without_hardware_plane() {
        let mut mgr = CursorManager::new();
        mgr.negotiate_render_mode(false);
        assert_eq!(mgr.state().render_mode, RenderMode::Software);
    }
}
