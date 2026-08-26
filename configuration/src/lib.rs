//! SHER-Display centralized configuration (spec section 37).
//!
//! One serializable struct, not scattered config files. Per-output
//! settings (resolution/refresh-rate/scale/orientation/primary) are keyed
//! by output name rather than modeled with `sher_display_outputs` types
//! directly — configuration and output management are peer crates
//! (section 52: small interfaces over coupling), so this crate defines its
//! own small `Orientation` rather than depending on `sher_display_outputs`
//! just for one enum.

use serde::{Deserialize, Serialize};
use sher_common::{Error, Result};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Landscape,
    Portrait,
    LandscapeFlipped,
    PortraitFlipped,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OutputSettings {
    pub resolution: (u32, u32),
    pub refresh_rate: u32,
    pub scale: f64,
    pub orientation: Orientation,
    pub primary: bool,
}

impl Default for OutputSettings {
    fn default() -> Self {
        OutputSettings {
            resolution: (1920, 1080),
            refresh_rate: 60,
            scale: 1.0,
            orientation: Orientation::default(),
            primary: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct WorkspaceBehavior {
    pub dynamic: bool,
    pub wrap_around: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WindowBehavior {
    pub focus_follows_mouse: bool,
    pub snap_enabled: bool,
    pub default_layout: String,
}

impl Default for WindowBehavior {
    fn default() -> Self {
        WindowBehavior {
            focus_follows_mouse: false,
            snap_enabled: true,
            default_layout: "floating".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AnimationSettings {
    pub enabled: bool,
    pub reduced_motion: bool,
    pub duration_scale: f64,
}

impl Default for AnimationSettings {
    fn default() -> Self {
        AnimationSettings {
            enabled: true,
            reduced_motion: false,
            duration_scale: 1.0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CursorSettings {
    pub theme: String,
    pub size_px: u32,
}

impl Default for CursorSettings {
    fn default() -> Self {
        CursorSettings {
            theme: "sher-default".to_string(),
            size_px: 24,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct AccessibilitySettings {
    pub high_contrast: bool,
    pub large_cursor: bool,
    pub reduced_motion: bool,
    pub screen_magnification: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct TouchpadGestures {
    pub natural_scroll: bool,
    pub tap_to_click: bool,
    pub three_finger_drag: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct PowerBehavior {
    pub reduce_animations_on_battery: bool,
    pub max_refresh_rate_on_battery: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct DisplayConfig {
    pub outputs: HashMap<String, OutputSettings>,
    pub workspace_behavior: WorkspaceBehavior,
    pub window_behavior: WindowBehavior,
    pub animation: AnimationSettings,
    pub cursor: CursorSettings,
    pub accessibility: AccessibilitySettings,
    pub keyboard_shortcuts: HashMap<String, String>,
    pub touchpad_gestures: TouchpadGestures,
    pub power_behavior: PowerBehavior,
}

impl DisplayConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_output(&mut self, name: impl Into<String>, settings: OutputSettings) {
        self.outputs.insert(name.into(), settings);
    }

    pub fn output(&self, name: &str) -> Option<&OutputSettings> {
        self.outputs.get(name)
    }

    pub fn bind_shortcut(&mut self, action: impl Into<String>, shortcut: impl Into<String>) {
        self.keyboard_shortcuts
            .insert(action.into(), shortcut.into());
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| Error::Unknown(e.to_string()))
    }

    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| Error::Unknown(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sensible() {
        let config = DisplayConfig::new();
        assert!(config.animation.enabled);
        assert!(!config.animation.reduced_motion);
        assert!(config.window_behavior.snap_enabled);
    }

    #[test]
    fn round_trips_through_json() {
        let mut config = DisplayConfig::new();
        config.set_output(
            "eDP-1",
            OutputSettings {
                resolution: (2560, 1600),
                scale: 2.0,
                primary: true,
                ..Default::default()
            },
        );
        config.bind_shortcut("workspace.switch.next", "Super+Right");

        let json = config.to_json().unwrap();
        let restored = DisplayConfig::from_json(&json).unwrap();

        assert_eq!(restored, config);
        assert_eq!(restored.output("eDP-1").unwrap().scale, 2.0);
    }

    #[test]
    fn invalid_json_is_rejected() {
        assert!(DisplayConfig::from_json("not json").is_err());
    }
}
