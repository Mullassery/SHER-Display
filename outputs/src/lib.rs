//! SHER-Display output management (spec section 11-14).
//!
//! This is the "Display Manager" box in the architecture diagram (section
//! 4): it turns raw connector state into desktop-facing concepts SHER-Kernel
//! and SHER-Graphics don't know about — logical position in an extended
//! desktop, per-output scale, and which output is primary.
//!
//! It deliberately does **not** own a `gpu_driver::GPUDriver`.
//! SHER-Graphics's `graphics_runtime::PresentationBridge` is already the
//! sole owner of connector/framebuffer/page-flip state (see that repo's
//! `GraphicsRuntime::register_connector`/`present_frame`) — instantiating a
//! second `GPUDriver` here would produce two independent, unsynchronized
//! views of the same display hardware, which is exactly the kind of
//! SHER-Graphics boundary violation this crate must not commit. Instead,
//! `OutputManager` mirrors `Connector`/`DisplayMode` facts that already
//! exist — registered with the real `GraphicsRuntime` by whoever wires
//! SHER-Display to SHER-Graphics (ROADMAP.md Phase 3) — into display-only
//! policy. `observe_connector`/`update_mode`/`handle_hotplug` never call
//! into any GPU driver; they only update this crate's own bookkeeping.

use gpu_driver::{Connector, DisplayMode};
use sher_common::{Error, ObjectId, Result};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Landscape,
    Portrait,
    LandscapeFlipped,
    PortraitFlipped,
}

#[derive(Clone, Debug)]
pub struct LogicalOutput {
    pub connector_id: ObjectId,
    pub name: String,
    /// Position in the extended desktop's global coordinate space.
    pub position: (i32, i32),
    pub scale: f64,
    pub orientation: Orientation,
    pub primary: bool,
    pub mirror_of: Option<ObjectId>,
    pub mode: Option<DisplayMode>,
}

/// True once no physical connector is registered, so the session has a
/// safe place to composite into instead of crashing (section 12).
const FALLBACK_OUTPUT_NAME: &str = "fallback-virtual";

pub struct OutputManager {
    logical: HashMap<ObjectId, LogicalOutput>,
    fallback_active: bool,
}

impl OutputManager {
    pub fn new() -> Self {
        let mut mgr = OutputManager { logical: HashMap::new(), fallback_active: false };
        mgr.ensure_fallback();
        mgr
    }

    /// Mirror a connector that has already been registered with the real
    /// `GraphicsRuntime` (SHER-Graphics) into a logical output. This does
    /// **not** register anything with a GPU driver — it only tracks
    /// desktop-facing policy for a connector SHER-Graphics already knows
    /// about. Does not assume identical resolution, DPI, or refresh rate
    /// across outputs (section 11).
    pub fn observe_connector(&mut self, connector: &Connector) -> ObjectId {
        let id = connector.id;
        let name = format!("{:?}-{}", connector.connector_type, id);

        let primary = self.logical.is_empty() || self.fallback_active;
        self.drop_fallback();

        self.logical.insert(
            id,
            LogicalOutput {
                connector_id: id,
                name,
                position: (0, 0),
                scale: 1.0,
                orientation: Orientation::Landscape,
                primary,
                mirror_of: None,
                mode: connector.current_mode.clone(),
            },
        );
        id
    }

    /// Records that this output's mode changed. The mode-set itself
    /// happens through SHER-Graphics; this only updates SHER-Display's
    /// mirror of that fact (e.g. for recomputing default DPI scale).
    pub fn update_mode(&mut self, output_id: &ObjectId, mode: DisplayMode) -> Result<()> {
        self.require_mut(output_id)?.mode = Some(mode);
        Ok(())
    }

    pub fn set_scale(&mut self, output_id: &ObjectId, scale: f64) -> Result<()> {
        if scale <= 0.0 {
            return Err(Error::Device("scale must be positive".to_string()));
        }
        self.require_mut(output_id)?.scale = scale;
        Ok(())
    }

    pub fn set_position(&mut self, output_id: &ObjectId, position: (i32, i32)) -> Result<()> {
        self.require_mut(output_id)?.position = position;
        Ok(())
    }

    pub fn set_orientation(&mut self, output_id: &ObjectId, orientation: Orientation) -> Result<()> {
        self.require_mut(output_id)?.orientation = orientation;
        Ok(())
    }

    pub fn set_primary(&mut self, output_id: &ObjectId) -> Result<()> {
        if !self.logical.contains_key(output_id) {
            return Err(Error::Device("output not found".to_string()));
        }
        for (id, output) in self.logical.iter_mut() {
            output.primary = id == output_id;
        }
        Ok(())
    }

    pub fn mirror(&mut self, output_id: &ObjectId, source_id: ObjectId) -> Result<()> {
        if !self.logical.contains_key(&source_id) {
            return Err(Error::Device("mirror source output not found".to_string()));
        }
        self.require_mut(output_id)?.mirror_of = Some(source_id);
        Ok(())
    }

    /// Never leaves the session with zero outputs and no primary display.
    /// Called after SHER-Kernel/SHER-Graphics report a hotplug event —
    /// this only updates SHER-Display's own logical-output bookkeeping
    /// (primary reassignment, fallback), never the GPU driver itself.
    pub fn handle_hotplug(&mut self, output_id: &ObjectId, connected: bool) -> Result<()> {
        if !connected {
            let was_primary = self.logical.get(output_id).map(|o| o.primary).unwrap_or(false);
            self.logical.remove(output_id);
            if was_primary {
                if let Some(next) = self.logical.keys().next().copied() {
                    self.set_primary(&next)?;
                } else {
                    self.ensure_fallback();
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, output_id: &ObjectId) -> Option<&LogicalOutput> {
        self.logical.get(output_id)
    }

    pub fn list(&self) -> Vec<&LogicalOutput> {
        self.logical.values().collect()
    }

    pub fn primary(&self) -> Option<&LogicalOutput> {
        self.logical.values().find(|o| o.primary)
    }

    fn ensure_fallback(&mut self) {
        if !self.logical.is_empty() {
            return;
        }
        let id = ObjectId::new();
        self.logical.insert(
            id,
            LogicalOutput {
                connector_id: id,
                name: FALLBACK_OUTPUT_NAME.to_string(),
                position: (0, 0),
                scale: 1.0,
                orientation: Orientation::Landscape,
                primary: true,
                mirror_of: None,
                mode: Some(DisplayMode { width: 1280, height: 720, refresh_rate: 60, clock: 74250 }),
            },
        );
        self.fallback_active = true;
    }

    fn drop_fallback(&mut self) {
        if self.fallback_active {
            self.logical.retain(|_, o| o.name != FALLBACK_OUTPUT_NAME);
            self.fallback_active = false;
        }
    }

    fn require_mut(&mut self, output_id: &ObjectId) -> Result<&mut LogicalOutput> {
        self.logical.get_mut(output_id).ok_or_else(|| Error::Device("output not found".to_string()))
    }
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpu_driver::{ConnectorStatus, ConnectorType};

    fn hdmi_connector() -> Connector {
        Connector {
            id: ObjectId::new(),
            connector_type: ConnectorType::HDMI,
            status: ConnectorStatus::Connected,
            supported_modes: vec![DisplayMode { width: 1920, height: 1080, refresh_rate: 60, clock: 148500 }],
            current_mode: None,
        }
    }

    #[test]
    fn starts_with_fallback_output_so_session_never_has_zero_outputs() {
        let mgr = OutputManager::new();
        assert_eq!(mgr.list().len(), 1);
        assert!(mgr.primary().is_some());
    }

    #[test]
    fn observing_real_connector_drops_fallback_and_becomes_primary() {
        let mut mgr = OutputManager::new();
        let connector = hdmi_connector();
        let id = mgr.observe_connector(&connector);

        assert_eq!(mgr.list().len(), 1);
        assert_eq!(mgr.primary().unwrap().connector_id, id);
    }

    #[test]
    fn disconnecting_primary_falls_back_to_next_or_virtual() {
        let mut mgr = OutputManager::new();
        let id = mgr.observe_connector(&hdmi_connector());

        mgr.handle_hotplug(&id, false).unwrap();

        assert_eq!(mgr.list().len(), 1);
        assert!(mgr.primary().is_some());
        assert_ne!(mgr.primary().unwrap().connector_id, id);
    }

    #[test]
    fn outputs_can_have_independent_scale_and_position() {
        let mut mgr = OutputManager::new();
        let laptop = mgr.observe_connector(&hdmi_connector());
        let external = mgr.observe_connector(&hdmi_connector());

        mgr.set_scale(&laptop, 2.0).unwrap();
        mgr.set_position(&external, (1920, 0)).unwrap();

        assert_eq!(mgr.get(&laptop).unwrap().scale, 2.0);
        assert_eq!(mgr.get(&external).unwrap().scale, 1.0);
        assert_eq!(mgr.get(&external).unwrap().position, (1920, 0));
    }
}
