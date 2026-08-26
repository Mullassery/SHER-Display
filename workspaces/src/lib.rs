//! SHER-Display virtual workspaces (spec section 10).
//!
//! Workspaces are global (not owned by a single output), but each output
//! tracks its own active workspace independently — the common multi-
//! monitor model where switching workspaces on one screen doesn't disturb
//! what's showing on another. Window and output references are plain
//! `ObjectId`s rather than crate dependencies, matching section 52's
//! "small interfaces, loosely coupled modules."

use sher_common::{Error, ObjectId, Result};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Workspace {
    pub id: ObjectId,
    pub name: String,
    pub index: usize,
    pub dynamic: bool,
    pub windows: Vec<ObjectId>,
}

pub struct WorkspaceManager {
    workspaces: HashMap<ObjectId, Workspace>,
    order: Vec<ObjectId>,
    active_per_output: HashMap<ObjectId, ObjectId>,
}

impl WorkspaceManager {
    /// Starts with a single static workspace — a session should never be
    /// without at least one place to put windows.
    pub fn new() -> Self {
        let mut mgr = WorkspaceManager {
            workspaces: HashMap::new(),
            order: Vec::new(),
            active_per_output: HashMap::new(),
        };
        mgr.create_workspace("Workspace 1", false);
        mgr
    }

    pub fn create_workspace(&mut self, name: impl Into<String>, dynamic: bool) -> ObjectId {
        let index = self.order.len();
        let workspace = Workspace {
            id: ObjectId::new(),
            name: name.into(),
            index,
            dynamic,
            windows: Vec::new(),
        };
        let id = workspace.id;
        self.workspaces.insert(id, workspace);
        self.order.push(id);
        id
    }

    pub fn delete_workspace(&mut self, id: &ObjectId) -> Result<()> {
        if self.workspaces.len() <= 1 {
            return Err(Error::Device(
                "cannot delete the last workspace".to_string(),
            ));
        }
        self.workspaces
            .remove(id)
            .ok_or_else(|| Error::Device("workspace not found".to_string()))?;
        self.order.retain(|w| w != id);
        self.active_per_output.retain(|_, active| active != id);
        for (i, id) in self.order.iter().enumerate() {
            if let Some(w) = self.workspaces.get_mut(id) {
                w.index = i;
            }
        }
        Ok(())
    }

    pub fn get(&self, id: &ObjectId) -> Option<&Workspace> {
        self.workspaces.get(id)
    }

    pub fn list(&self) -> Vec<&Workspace> {
        self.order
            .iter()
            .filter_map(|id| self.workspaces.get(id))
            .collect()
    }

    pub fn switch(&mut self, output_id: ObjectId, workspace_id: ObjectId) -> Result<()> {
        if !self.workspaces.contains_key(&workspace_id) {
            return Err(Error::Device("workspace not found".to_string()));
        }
        self.active_per_output.insert(output_id, workspace_id);
        Ok(())
    }

    pub fn active_for_output(&self, output_id: &ObjectId) -> Option<ObjectId> {
        self.active_per_output.get(output_id).copied()
    }

    /// Assigns a window and, if this was the last dynamic workspace in the
    /// sequence, appends a fresh empty one — GNOME-style dynamic
    /// workspaces where there's always one spare at the end.
    pub fn assign_window(&mut self, workspace_id: &ObjectId, window_id: ObjectId) -> Result<()> {
        let is_last_dynamic = {
            let workspace = self
                .workspaces
                .get_mut(workspace_id)
                .ok_or_else(|| Error::Device("workspace not found".to_string()))?;
            if !workspace.windows.contains(&window_id) {
                workspace.windows.push(window_id);
            }
            workspace.dynamic && self.order.last() == Some(workspace_id)
        };
        if is_last_dynamic {
            self.create_workspace(format!("Workspace {}", self.order.len() + 1), true);
        }
        Ok(())
    }

    pub fn move_window(
        &mut self,
        window_id: &ObjectId,
        from: &ObjectId,
        to: &ObjectId,
    ) -> Result<()> {
        if !self.workspaces.contains_key(to) {
            return Err(Error::Device("destination workspace not found".to_string()));
        }
        let source = self
            .workspaces
            .get_mut(from)
            .ok_or_else(|| Error::Device("source workspace not found".to_string()))?;
        source.windows.retain(|w| w != window_id);
        self.assign_window(to, *window_id)
    }

    pub fn windows_in(&self, workspace_id: &ObjectId) -> &[ObjectId] {
        self.workspaces
            .get(workspace_id)
            .map(|w| w.windows.as_slice())
            .unwrap_or(&[])
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cannot_delete_the_last_workspace() {
        let mut mgr = WorkspaceManager::new();
        let only = mgr.list()[0].id;
        assert!(mgr.delete_workspace(&only).is_err());
    }

    #[test]
    fn outputs_switch_workspaces_independently() {
        let mut mgr = WorkspaceManager::new();
        let ws2 = mgr.create_workspace("Workspace 2", false);
        let laptop = ObjectId::new();
        let external = ObjectId::new();
        let ws1 = mgr.list()[0].id;

        mgr.switch(laptop, ws1).unwrap();
        mgr.switch(external, ws2).unwrap();

        assert_eq!(mgr.active_for_output(&laptop), Some(ws1));
        assert_eq!(mgr.active_for_output(&external), Some(ws2));
    }

    #[test]
    fn dynamic_workspaces_grow_when_last_one_fills() {
        let mut mgr = WorkspaceManager::new();
        let dyn_ws = mgr.create_workspace("Dynamic 1", true);
        let before = mgr.list().len();

        mgr.assign_window(&dyn_ws, ObjectId::new()).unwrap();

        assert_eq!(mgr.list().len(), before + 1);
    }

    #[test]
    fn moving_a_window_updates_both_workspaces() {
        let mut mgr = WorkspaceManager::new();
        let ws1 = mgr.list()[0].id;
        let ws2 = mgr.create_workspace("Workspace 2", false);
        let window = ObjectId::new();

        mgr.assign_window(&ws1, window).unwrap();
        mgr.move_window(&window, &ws1, &ws2).unwrap();

        assert!(!mgr.windows_in(&ws1).contains(&window));
        assert!(mgr.windows_in(&ws2).contains(&window));
    }
}
