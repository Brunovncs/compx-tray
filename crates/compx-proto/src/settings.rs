//! Settings of the active onboard profile, as exchanged with the panel.
//! Reading and writing them lives in `eeprom` (feature `hid`).

use serde::{Deserialize, Serialize};

use crate::sensor::LiftOff;

pub const MAX_STAGES: u8 = 8;
pub const DEBOUNCE_MS: [u8; 7] = [0, 1, 2, 4, 8, 15, 20];

/// Sensor performance mode (EEPROM 0xB9).
pub mod sensor_mode {
    pub const BASIC: u8 = 0;
    pub const COMPETITIVE: u8 = 1;
    /// Only on newer mice (CID 1).
    pub const COMPETITIVE_MAX: u8 = 2;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dpi {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiftOffOption {
    pub raw: u8,
    pub tenths_mm: u8,
}

impl From<LiftOff> for LiftOffOption {
    fn from(l: LiftOff) -> Self {
        Self { raw: l.raw, tenths_mm: l.tenths_mm }
    }
}

/// Current values plus what this mouse accepts, so the panel needs no device tables of its own.
/// `None` means the value could not be read (bad crc) or does not exist on this mouse.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MouseSettings {
    pub polling_hz: Option<u16>,
    pub polling_options: Vec<u16>,
    /// Empty when the sensor is unknown: its DPI encoding cannot be trusted.
    pub stages: Vec<Dpi>,
    pub stage_count: u8,
    /// 0-based stage in use.
    pub current_stage: u8,
    pub dpi_min: u32,
    /// `(up_to, step)` segments; empty when DPI cannot be edited.
    pub dpi_steps: Vec<(u32, u32)>,
    pub lift_off: Option<u8>,
    pub lift_off_options: Vec<LiftOffOption>,
    pub debounce_ms: Option<u8>,
    pub motion_sync: Option<bool>,
    pub angle_snap: Option<bool>,
    pub ripple: Option<bool>,
    pub sensor_mode: Option<u8>,
    pub max_sensor_mode: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SettingChange {
    Polling { hz: u16 },
    LiftOff { raw: u8 },
    CurrentStage { index: u8 },
    StageCount { count: u8 },
    /// Sets X and Y to the same value.
    StageDpi { index: u8, dpi: u32 },
    Debounce { ms: u8 },
    MotionSync { on: bool },
    AngleSnap { on: bool },
    Ripple { on: bool },
    SensorMode { mode: u8 },
}
