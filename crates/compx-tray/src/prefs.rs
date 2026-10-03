//! User preferences in `HKCU\Software\compx-tray`, plus the "Start with Windows" entry.

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

use compx_proto::ipc::Prefs;
use compx_proto::rules::{self, RuleMode, MAX_PROFILES};

const APP_KEY: &str = r"Software\compx-tray";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "COMPX Tray";

fn app_key() -> std::io::Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).create_subkey(APP_KEY).map(|(k, _)| k)
}

fn get<T: winreg::types::FromRegValue>(name: &str) -> Option<T> {
    app_key().ok()?.get_value(name).ok()
}

fn set<T: winreg::types::ToRegValue>(name: &str, value: &T) {
    if let Ok(k) = app_key() {
        let _ = k.set_value(name, value);
    }
}

fn get_flag(name: &str) -> Option<bool> {
    get::<u32>(name).map(|v| v != 0)
}

pub fn load() -> Prefs {
    // One REG_SZ with a name per line: REG_MULTI_SZ cannot hold empty entries.
    let names: String = get("profile_names").unwrap_or_default();
    let mut names = names.lines();
    Prefs {
        // `auto_valorant` is the old name, from when Valorant was the only rule.
        auto_switch: get_flag("auto_switch").or_else(|| get_flag("auto_valorant")).unwrap_or(false),
        rule_mode: get::<String>("rule_mode").and_then(|s| RuleMode::parse(&s)).unwrap_or_default(),
        default_profile: get::<u32>("default_profile").map_or(0, |v| v.min(MAX_PROFILES as u32 - 1) as u8),
        rules: get::<String>("rules").map(|t| rules::parse(&t)).unwrap_or_default(),
        profile_names: (0..MAX_PROFILES).map(|_| names.next().unwrap_or_default().to_owned()).collect(),
        osd: get_flag("osd").unwrap_or(true),
        hotkeys: get_flag("hotkeys").unwrap_or(true),
        autostart: autostart(),
    }
}

pub fn save(p: &Prefs) {
    set("auto_switch", &(p.auto_switch as u32));
    set("rule_mode", &p.rule_mode.as_str());
    set("default_profile", &(p.default_profile as u32));
    set("rules", &rules::encode(&p.rules));
    set("profile_names", &p.profile_names.join("\n"));
    set("osd", &(p.osd as u32));
    set("hotkeys", &(p.hotkeys as u32));
}

/// Where the panel finds the local channel.
pub fn set_ipc_endpoint(port: u16, token: &str) {
    set("ipc_port", &(port as u32));
    set("ipc_token", &token);
}

fn run_key() -> std::io::Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE)
}

pub fn autostart() -> bool {
    run_key().and_then(|k| k.get_value::<String, _>(RUN_VALUE)).is_ok()
}

pub fn set_autostart(on: bool) {
    let Ok(k) = run_key() else { return };
    if !on {
        let _ = k.delete_value(RUN_VALUE);
    } else if let Ok(exe) = std::env::current_exe() {
        let _ = k.set_value(RUN_VALUE, &format!("\"{}\"", exe.display()));
    }
}

/// On the first run, turns autostart on; afterwards only fixes the path if the exe moved.
pub fn init() {
    if get_flag("initialized").is_none() {
        set_autostart(true);
        set("initialized", &1u32);
    } else if autostart() {
        set_autostart(true);
    }
}
