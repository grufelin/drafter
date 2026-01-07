use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub version: u32,
    pub config: PlanConfig,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanConfig {
    pub layout: String,
    pub keymap_format: u32,
    pub keymap: String,
    pub wpm_target: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Wait {
        ms: u64,
    },
    Modifiers {
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    },
    Key {
        keycode: u32,
        state: KeyState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
    Pressed,
    Released,
}
