//! SHER-Display security boundary (spec section 46).
//!
//! Every privileged operation SHER-Display exposes — capture, recording,
//! input injection, clipboard access, window inspection, global
//! shortcuts, display configuration, remote display, accessibility
//! privileges — is gated by a time-bound grant, mirroring SHER-Kernel's
//! capability model (see SHER-Kernel's `CapabilityGrant`/`PermissionTier`)
//! rather than inventing a second permission philosophy for the display
//! layer. No permission lasts forever; callers pass in the current time
//! rather than this crate reading the clock, so grant expiry is testable.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Permission {
    ScreenCapture,
    ScreenRecording,
    InputInjection,
    ClipboardAccess,
    WindowInspection,
    GlobalShortcuts,
    DisplayConfiguration,
    RemoteDisplay,
    AccessibilityPrivileges,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionTier {
    Low,
    Medium,
    High,
    Critical,
}

impl PermissionTier {
    pub fn duration_secs(self) -> u64 {
        match self {
            PermissionTier::Low => 60 * 60,
            PermissionTier::Medium => 24 * 60 * 60,
            PermissionTier::High => 2 * 60 * 60,
            PermissionTier::Critical => 30 * 60,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Grant {
    pub app_id: String,
    pub permission: Permission,
    pub tier: PermissionTier,
    pub granted_at: u64,
    pub expires_at: u64,
}

#[derive(Default)]
pub struct PermissionManager {
    grants: HashMap<(String, Permission), Grant>,
}

impl PermissionManager {
    pub fn new() -> Self {
        PermissionManager {
            grants: HashMap::new(),
        }
    }

    pub fn grant(
        &mut self,
        app_id: impl Into<String>,
        permission: Permission,
        tier: PermissionTier,
        now: u64,
    ) {
        let app_id = app_id.into();
        let grant = Grant {
            app_id: app_id.clone(),
            permission,
            tier,
            granted_at: now,
            expires_at: now + tier.duration_secs(),
        };
        self.grants.insert((app_id, permission), grant);
    }

    pub fn revoke(&mut self, app_id: &str, permission: Permission) {
        self.grants.remove(&(app_id.to_string(), permission));
    }

    /// Expired grants are treated as absent, not specially reported —
    /// fail secure (SHER-Kernel's zero-trust principle: on doubt, deny).
    pub fn check(&self, app_id: &str, permission: Permission, now: u64) -> bool {
        self.grants
            .get(&(app_id.to_string(), permission))
            .map(|g| now < g.expires_at)
            .unwrap_or(false)
    }

    pub fn active_grants(&self, now: u64) -> Vec<&Grant> {
        self.grants
            .values()
            .filter(|g| now < g.expires_at)
            .collect()
    }

    /// Drops expired entries. Not called implicitly by `check` so that
    /// read-only queries never mutate state.
    pub fn prune_expired(&mut self, now: u64) {
        self.grants.retain(|_, g| now < g.expires_at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ungranted_permission_is_denied() {
        let mgr = PermissionManager::new();
        assert!(!mgr.check("app.example", Permission::ScreenCapture, 1_000));
    }

    #[test]
    fn granted_permission_allowed_until_expiry() {
        let mut mgr = PermissionManager::new();
        mgr.grant(
            "app.example",
            Permission::ScreenCapture,
            PermissionTier::Critical,
            1_000,
        );

        assert!(mgr.check("app.example", Permission::ScreenCapture, 1_500));
        assert!(!mgr.check(
            "app.example",
            Permission::ScreenCapture,
            1_000 + PermissionTier::Critical.duration_secs()
        ));
    }

    #[test]
    fn revoke_removes_grant_immediately() {
        let mut mgr = PermissionManager::new();
        mgr.grant(
            "app.example",
            Permission::ClipboardAccess,
            PermissionTier::High,
            1_000,
        );
        mgr.revoke("app.example", Permission::ClipboardAccess);
        assert!(!mgr.check("app.example", Permission::ClipboardAccess, 1_001));
    }

    #[test]
    fn a_grant_for_one_app_does_not_leak_to_another() {
        let mut mgr = PermissionManager::new();
        mgr.grant(
            "app.trusted",
            Permission::InputInjection,
            PermissionTier::Low,
            1_000,
        );
        assert!(!mgr.check("app.other", Permission::InputInjection, 1_001));
    }
}
