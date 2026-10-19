//! SHER-Display compositor core (spec section 5-6).
//!
//! Implements the frame pipeline up through composition: surface commits
//! feed the scene graph, damage is tracked per output, and the per-output
//! scheduler (`scheduler::FrameScheduler`) decides whether a frame is due
//! and whether it needs a full redraw or just the damaged region. Each
//! output has its own refresh interval, deadline, and tick counter, so
//! mixed-refresh layouts (e.g. 60 Hz + 144 Hz) advance independently
//! instead of being paced by one global frame clock. The next two pipeline
//! stages in section 6 —
//! GPU rendering and actual display output — are SHER-Graphics's and
//! SHER-Kernel's jobs respectively; this crate stops at producing a
//! `FrameReport` describing what would need to be rendered. Wiring that
//! report into `graphics_runtime` is follow-up work, not scaffolded here,
//! so this crate isn't left depending on a GPU abstraction it can't yet
//! exercise.

mod scheduler;

pub use scheduler::{refresh_interval, FrameScheduler, OutputTick, OutputTimer};

use sher_common::{ObjectId, Result};
use sher_display_scene::{Rect, SceneGraph, SceneNode};
use sher_display_surfaces::SurfaceManager;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct FrameReport {
    pub output_id: ObjectId,
    pub frame_number: u64,
    pub full_redraw: bool,
    pub damage: Vec<Rect>,
    pub composited_nodes: usize,
}

/// Render-loop state that is not timing-related. Timing lives in the
/// `FrameScheduler` so scheduling is pure and testable without a clock.
struct OutputRenderState {
    frame_number: u64,
    force_full_redraw: bool,
}

impl OutputRenderState {
    fn new() -> Self {
        OutputRenderState {
            frame_number: 0,
            force_full_redraw: true,
        }
    }
}

pub struct Compositor {
    scene: SceneGraph,
    surfaces: SurfaceManager,
    scheduler: FrameScheduler,
    render_state: HashMap<ObjectId, OutputRenderState>,
}

impl Compositor {
    pub fn new() -> Self {
        Compositor {
            scene: SceneGraph::new(),
            surfaces: SurfaceManager::new(),
            scheduler: FrameScheduler::new(),
            render_state: HashMap::new(),
        }
    }

    pub fn scene(&self) -> &SceneGraph {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut SceneGraph {
        &mut self.scene
    }

    pub fn surfaces(&self) -> &SurfaceManager {
        &self.surfaces
    }

    pub fn surfaces_mut(&mut self) -> &mut SurfaceManager {
        &mut self.surfaces
    }

    pub fn register_output(&mut self, output_id: ObjectId, refresh_hz: u32) {
        self.scheduler.register_output(output_id, refresh_hz);
        self.render_state
            .insert(output_id, OutputRenderState::new());
    }

    pub fn unregister_output(&mut self, output_id: &ObjectId) {
        self.scheduler.unregister_output(output_id);
        self.render_state.remove(output_id);
    }

    /// Change one output's refresh rate without disturbing any other
    /// output's deadline or tick counter.
    pub fn set_refresh_rate(&mut self, output_id: &ObjectId, refresh_hz: u32) -> bool {
        self.scheduler.set_refresh_rate(output_id, refresh_hz)
    }

    pub fn request_full_redraw(&mut self, output_id: &ObjectId) {
        if let Some(state) = self.render_state.get_mut(output_id) {
            state.force_full_redraw = true;
        }
    }

    pub fn scheduler(&self) -> &FrameScheduler {
        &self.scheduler
    }

    pub fn next_deadline(&self, output_id: &ObjectId) -> Option<Duration> {
        self.scheduler.next_deadline(output_id)
    }

    /// Advance the shared clock. Each output is evaluated against its own
    /// deadline; a due output produces a `FrameReport` only if there's
    /// damage (or a full redraw was requested) — VRR-friendly: an output
    /// with nothing to draw simply doesn't get a report (section 14's
    /// "adaptive synchronization"). One output never consumes another's
    /// deadline.
    pub fn tick(&mut self, dt: Duration) -> Vec<FrameReport> {
        let mut reports = Vec::new();

        for output_tick in self.scheduler.advance(dt) {
            let output_id = output_tick.output_id;
            let has_damage = self.scene.has_damage(&output_id);
            let state = self
                .render_state
                .get_mut(&output_id)
                .expect("every scheduler output is registered in render_state at the same time");

            if !has_damage && !state.force_full_redraw {
                continue;
            }

            let full_redraw = state.force_full_redraw;
            state.force_full_redraw = false;
            state.frame_number += 1;
            let frame_number = state.frame_number;

            reports.push(self.compose(output_id, frame_number, full_redraw));
        }

        reports
    }

    fn compose(
        &mut self,
        output_id: ObjectId,
        frame_number: u64,
        full_redraw: bool,
    ) -> FrameReport {
        let nodes: Vec<&SceneNode> = self.scene.z_ordered_for_output(&output_id);
        let mut damage = Vec::new();
        let mut node_ids = Vec::new();

        for node in &nodes {
            if let Some(region) = node.damage {
                damage.push(region);
            }
            node_ids.push(node.id);
        }

        for id in &node_ids {
            self.scene.clear_damage(id);
        }

        FrameReport {
            output_id,
            frame_number,
            full_redraw,
            damage,
            composited_nodes: node_ids.len(),
        }
    }

    pub fn commit_surface(&mut self, surface_id: &ObjectId, node_id: &ObjectId) -> Result<()> {
        let damage_regions = self.surfaces.commit(surface_id)?;
        for region in damage_regions {
            self.scene.mark_damage(node_id, region)?;
        }
        self.surfaces.ack_frame_callback(surface_id)
    }
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sher_display_scene::NodeKind;
    use sher_display_surfaces::SurfaceRole;

    #[test]
    fn first_tick_forces_full_redraw_even_without_damage() {
        let mut comp = Compositor::new();
        let output = ObjectId::new();
        comp.register_output(output, 60);

        let reports = comp.tick(Duration::from_millis(17));
        assert_eq!(reports.len(), 1);
        assert!(reports[0].full_redraw);
    }

    #[test]
    fn no_damage_after_first_frame_skips_subsequent_frames() {
        let mut comp = Compositor::new();
        let output = ObjectId::new();
        comp.register_output(output, 60);
        comp.tick(Duration::from_millis(17));

        let reports = comp.tick(Duration::from_millis(17));
        assert!(reports.is_empty());
    }

    #[test]
    fn committing_damaged_surface_produces_a_frame() {
        let mut comp = Compositor::new();
        let output = ObjectId::new();
        comp.register_output(output, 60);
        comp.tick(Duration::from_millis(17));

        let client = ObjectId::new();
        let surface_id = comp
            .surfaces_mut()
            .create_surface(client, SurfaceRole::Toplevel);
        let mut node = SceneNode::new(NodeKind::Window);
        node.output_id = Some(output);
        let node_id = comp.scene_mut().insert(node);

        comp.surfaces_mut()
            .damage(&surface_id, Rect::new(0.0, 0.0, 10.0, 10.0))
            .unwrap();
        comp.commit_surface(&surface_id, &node_id).unwrap();

        let reports = comp.tick(Duration::from_millis(17));
        assert_eq!(reports.len(), 1);
        assert!(!reports[0].full_redraw);
        assert_eq!(reports[0].damage.len(), 1);
    }

    #[test]
    fn tick_before_refresh_interval_elapses_produces_nothing() {
        let mut comp = Compositor::new();
        let output = ObjectId::new();
        comp.register_output(output, 60);
        comp.tick(Duration::from_millis(17));

        let reports = comp.tick(Duration::from_millis(2));
        assert!(reports.is_empty());
    }

    #[test]
    fn mixed_refresh_outputs_produce_frames_without_gating_each_other() {
        let mut comp = Compositor::new();
        let slow = ObjectId::new();
        let fast = ObjectId::new();
        comp.register_output(slow, 60);
        comp.register_output(fast, 144);

        // 7ms: only the 144 Hz output's first deadline is reached.
        let reports = comp.tick(Duration::from_millis(7));
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].output_id, fast);
        assert!(reports[0].full_redraw);

        // The 60 Hz output still has its original deadline and has not
        // rendered a frame.
        assert_eq!(comp.next_deadline(&slow), Some(crate::refresh_interval(60)));
        assert_eq!(comp.scheduler().ticks(&slow), Some(0));

        // 10ms later (t=17ms) the 60 Hz output's first (forced) frame is
        // due. The 144 Hz output also reached another deadline, but has
        // no damage, so it correctly renders nothing — while still
        // advancing its own tick counter.
        let reports = comp.tick(Duration::from_millis(10));
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].output_id, slow);
        assert_eq!(comp.scheduler().ticks(&slow), Some(1));
        assert_eq!(comp.scheduler().ticks(&fast), Some(2));

        // At t=27ms only the fast output's next deadline is due; forcing
        // a redraw for it produces a fast frame without touching slow.
        comp.request_full_redraw(&fast);
        let reports = comp.tick(Duration::from_millis(10));
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].output_id, fast);
        assert_eq!(comp.scheduler().ticks(&slow), Some(1));
        assert_eq!(comp.scheduler().ticks(&fast), Some(3));
    }
}
