//! SHER-Display compositor core (spec section 5-6).
//!
//! Implements the frame pipeline up through composition: surface commits
//! feed the scene graph, damage is tracked per output, and the scheduler
//! decides whether a frame is due and whether it needs a full redraw or
//! just the damaged region. The next two pipeline stages in section 6 —
//! GPU rendering and actual display output — are SHER-Graphics's and
//! SHER-Kernel's jobs respectively; this crate stops at producing a
//! `FrameReport` describing what would need to be rendered. Wiring that
//! report into `graphics_runtime` is follow-up work, not scaffolded here,
//! so this crate isn't left depending on a GPU abstraction it can't yet
//! exercise.

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

struct OutputSchedule {
    refresh_interval: Duration,
    elapsed_since_last_frame: Duration,
    frame_number: u64,
    force_full_redraw: bool,
}

impl OutputSchedule {
    fn new(refresh_hz: u32) -> Self {
        OutputSchedule {
            refresh_interval: Duration::from_secs_f64(1.0 / refresh_hz.max(1) as f64),
            elapsed_since_last_frame: Duration::ZERO,
            frame_number: 0,
            force_full_redraw: true,
        }
    }
}

pub struct Compositor {
    scene: SceneGraph,
    surfaces: SurfaceManager,
    schedules: HashMap<ObjectId, OutputSchedule>,
}

impl Compositor {
    pub fn new() -> Self {
        Compositor {
            scene: SceneGraph::new(),
            surfaces: SurfaceManager::new(),
            schedules: HashMap::new(),
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
        self.schedules
            .insert(output_id, OutputSchedule::new(refresh_hz));
    }

    pub fn unregister_output(&mut self, output_id: &ObjectId) {
        self.schedules.remove(output_id);
    }

    pub fn request_full_redraw(&mut self, output_id: &ObjectId) {
        if let Some(schedule) = self.schedules.get_mut(output_id) {
            schedule.force_full_redraw = true;
        }
    }

    /// Advance the clock for every registered output and, for each one
    /// whose refresh interval has elapsed, produce a `FrameReport` if
    /// there's damage (or a full redraw was requested) — VRR-friendly:
    /// an output with nothing to draw simply doesn't get a report
    /// (section 14's "adaptive synchronization").
    pub fn tick(&mut self, dt: Duration) -> Vec<FrameReport> {
        let mut reports = Vec::new();
        let output_ids: Vec<ObjectId> = self.schedules.keys().copied().collect();

        for output_id in output_ids {
            let due = {
                let schedule = self.schedules.get_mut(&output_id).unwrap();
                schedule.elapsed_since_last_frame += dt;
                schedule.elapsed_since_last_frame >= schedule.refresh_interval
            };
            if !due {
                continue;
            }

            let has_damage = self.scene.has_damage(&output_id);
            let schedule = self.schedules.get_mut(&output_id).unwrap();
            if !has_damage && !schedule.force_full_redraw {
                schedule.elapsed_since_last_frame = Duration::ZERO;
                continue;
            }

            let full_redraw = schedule.force_full_redraw;
            schedule.force_full_redraw = false;
            schedule.elapsed_since_last_frame = Duration::ZERO;
            schedule.frame_number += 1;
            let frame_number = schedule.frame_number;

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
}
