//! SHER-Display scene graph (spec section 7).
//!
//! Owns the shared geometry primitives (`Point`, `Size`, `Rect`, `Transform`)
//! used by every other SHER-Display crate, and the scene node tree that
//! composition reads to build a frame. Surfaces, windows, cursor and
//! animation state are stored elsewhere; this crate only tracks *where and
//! how* a node participates in composition, not its domain-specific state.

use sher_common::{Error, ObjectId, Result};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Rect { origin: Point { x, y }, size: Size { width, height } }
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let x0 = self.origin.x.min(other.origin.x);
        let y0 = self.origin.y.min(other.origin.y);
        let x1 = (self.origin.x + self.size.width).max(other.origin.x + other.size.width);
        let y1 = (self.origin.y + self.size.height).max(other.origin.y + other.size.height);
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transform {
    pub scale: f64,
    pub rotation_degrees: f64,
}

impl Transform {
    pub fn identity() -> Self {
        Transform { scale: 1.0, rotation_degrees: 0.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Window,
    Dialog,
    Menu,
    Popover,
    Tooltip,
    Notification,
    Panel,
    Desktop,
    Wallpaper,
    Overlay,
    Cursor,
    SystemUi,
}

#[derive(Clone, Debug)]
pub struct SceneNode {
    pub id: ObjectId,
    pub kind: NodeKind,
    pub position: Point,
    pub size: Size,
    pub transform: Transform,
    pub opacity: f32,
    pub z_order: i32,
    pub visible: bool,
    pub input_region: Option<Rect>,
    pub damage: Option<Rect>,
    pub output_id: Option<ObjectId>,
}

impl SceneNode {
    pub fn new(kind: NodeKind) -> Self {
        SceneNode {
            id: ObjectId::new(),
            kind,
            position: Point::default(),
            size: Size::default(),
            transform: Transform::identity(),
            opacity: 1.0,
            z_order: 0,
            visible: true,
            input_region: None,
            damage: None,
            output_id: None,
        }
    }

    pub fn bounds(&self) -> Rect {
        Rect { origin: self.position, size: self.size }
    }
}

/// The scene graph: a flat, z-ordered set of nodes. Kept flat rather than a
/// true tree for now — nesting (e.g. popovers owned by a window) is tracked
/// by domain crates via `ObjectId` references, not by parent/child edges
/// here. Revisit if composition needs real subtree transforms.
#[derive(Default)]
pub struct SceneGraph {
    nodes: HashMap<ObjectId, SceneNode>,
}

impl SceneGraph {
    pub fn new() -> Self {
        SceneGraph { nodes: HashMap::new() }
    }

    pub fn insert(&mut self, node: SceneNode) -> ObjectId {
        let id = node.id;
        self.nodes.insert(id, node);
        id
    }

    pub fn remove(&mut self, id: &ObjectId) -> Result<()> {
        self.nodes
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| Error::Device("scene node not found".to_string()))
    }

    pub fn get(&self, id: &ObjectId) -> Option<&SceneNode> {
        self.nodes.get(id)
    }

    pub fn get_mut(&mut self, id: &ObjectId) -> Option<&mut SceneNode> {
        self.nodes.get_mut(id)
    }

    pub fn mark_damage(&mut self, id: &ObjectId, region: Rect) -> Result<()> {
        let node = self
            .nodes
            .get_mut(id)
            .ok_or_else(|| Error::Device("scene node not found".to_string()))?;
        node.damage = Some(match node.damage {
            Some(existing) => existing.union(&region),
            None => region,
        });
        Ok(())
    }

    pub fn clear_damage(&mut self, id: &ObjectId) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.damage = None;
        }
    }

    /// Nodes touching the given output, back-to-front, for composition.
    pub fn z_ordered_for_output(&self, output_id: &ObjectId) -> Vec<&SceneNode> {
        let mut nodes: Vec<&SceneNode> = self
            .nodes
            .values()
            .filter(|n| n.visible && n.output_id.as_ref() == Some(output_id))
            .collect();
        nodes.sort_by_key(|n| n.z_order);
        nodes
    }

    /// True if any node on the output carries pending damage — the signal
    /// composition uses to decide whether the frame needs a full redraw or
    /// can be skipped entirely (section 6).
    pub fn has_damage(&self, output_id: &ObjectId) -> bool {
        self.nodes
            .values()
            .any(|n| n.output_id.as_ref() == Some(output_id) && n.damage.is_some())
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn z_order_sorts_back_to_front() {
        let mut graph = SceneGraph::new();
        let output = ObjectId::new();

        let mut back = SceneNode::new(NodeKind::Wallpaper);
        back.z_order = 0;
        back.output_id = Some(output);
        let back_id = graph.insert(back);

        let mut front = SceneNode::new(NodeKind::Window);
        front.z_order = 10;
        front.output_id = Some(output);
        let front_id = graph.insert(front);

        let ordered = graph.z_ordered_for_output(&output);
        assert_eq!(ordered[0].id, back_id);
        assert_eq!(ordered[1].id, front_id);
    }

    #[test]
    fn damage_accumulates_via_union() {
        let mut graph = SceneGraph::new();
        let node = SceneNode::new(NodeKind::Window);
        let id = graph.insert(node);

        graph.mark_damage(&id, Rect::new(0.0, 0.0, 10.0, 10.0)).unwrap();
        graph.mark_damage(&id, Rect::new(5.0, 5.0, 10.0, 10.0)).unwrap();

        let damage = graph.get(&id).unwrap().damage.unwrap();
        assert_eq!(damage, Rect::new(0.0, 0.0, 15.0, 15.0));
    }

    #[test]
    fn hidden_nodes_excluded_from_composition() {
        let mut graph = SceneGraph::new();
        let output = ObjectId::new();
        let mut node = SceneNode::new(NodeKind::Notification);
        node.output_id = Some(output);
        node.visible = false;
        graph.insert(node);

        assert!(graph.z_ordered_for_output(&output).is_empty());
    }
}
