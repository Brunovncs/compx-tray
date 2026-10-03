//! Messages between the tray (owner of the HID handle) and the panel: one JSON object per line
//! over 127.0.0.1. The tray stores the port and a random token under `HKCU\Software\compx-tray`;
//! a client must send [`PanelMessage::Hello`] with that token first or it is dropped. This keeps
//! other local users and web pages (which can POST to localhost) from driving the mouse.

use serde::{Deserialize, Serialize};

use crate::info::{Battery, DeviceInfo};
use crate::rules::{Rule, RuleMode};
use crate::settings::{MouseSettings, SettingChange};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MouseState {
    Disconnected,
    /// Receiver present, mouse not answering.
    Asleep,
    Ready,
    /// ATK HUB is running (e.g. updating firmware), so the device is left alone.
    Paused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub mouse: MouseState,
    pub profile: Option<u8>,
    pub battery: Option<Battery>,
    pub device: Option<DeviceInfo>,
}

impl Status {
    pub const DISCONNECTED: Status = Status { mouse: MouseState::Disconnected, profile: None, battery: None, device: None };
    pub const PAUSED: Status = Status { mouse: MouseState::Paused, profile: None, battery: None, device: None };
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    pub auto_switch: bool,
    pub rule_mode: RuleMode,
    /// Profile used when no rule matches.
    pub default_profile: u8,
    pub rules: Vec<Rule>,
    /// One per onboard profile; empty means "Profile N".
    pub profile_names: Vec<String>,
    /// Show a toast when the profile changes.
    pub osd: bool,
    /// Ctrl+Alt+Shift+1…N switch profiles.
    pub hotkeys: bool,
    pub autostart: bool,
}

/// A saved copy of one profile's settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    /// File name inside the backups folder.
    pub name: String,
    pub model: String,
    pub profile: u8,
    /// Seconds since the Unix epoch.
    pub created: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TrayMessage {
    Status { status: Status },
    Prefs { prefs: Prefs },
    Settings { settings: MouseSettings },
    /// Reading or writing settings failed; a fresh `Settings` usually follows after a write.
    SettingsError { message: String },
    Backups { backups: Vec<Backup> },
    /// Result of a one-off action (backup, restore).
    Notice { ok: bool, message: String },
    /// Bring the panel window to the front.
    Focus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PanelMessage {
    Hello { token: String },
    Focus,
    SetProfile { index: u8 },
    SetPrefs { prefs: Prefs },
    ReadSettings,
    Write { change: SettingChange },
    Backup,
    Restore { name: String },
    DeleteBackup { name: String },
}

/// Serializes a message as one line, newline included.
pub fn line<T: Serialize>(msg: &T) -> String {
    let mut s = serde_json::to_string(msg).expect("IPC messages always serialize");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        let msg = PanelMessage::Write { change: SettingChange::StageDpi { index: 2, dpi: 1600 } };
        let text = line(&msg);
        assert_eq!(text, "{\"type\":\"write\",\"change\":{\"kind\":\"stageDpi\",\"index\":2,\"dpi\":1600}}\n");
        assert_eq!(serde_json::from_str::<PanelMessage>(&text).unwrap(), msg);

        let status = TrayMessage::Status { status: Status::DISCONNECTED };
        assert_eq!(serde_json::from_str::<TrayMessage>(&line(&status)).unwrap(), status);
    }
}
