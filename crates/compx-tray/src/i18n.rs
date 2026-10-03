//! Tray strings in English and Brazilian Portuguese, picked from the Windows UI language.

use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;

pub struct Strings {
    pub open_panel: &'static str,
    pub searching: &'static str,
    pub disconnected: &'static str,
    pub asleep: &'static str,
    pub paused: &'static str,
    pub battery: &'static str,
    pub charging: &'static str,
    pub profile: &'static str,
    pub auto_switch: &'static str,
    pub autostart: &'static str,
    pub quit: &'static str,
    pub low_battery: &'static str,
}

const EN: Strings = Strings {
    open_panel: "Open panel",
    searching: "Looking for the mouse…",
    disconnected: "Mouse disconnected",
    asleep: "Mouse not responding (asleep?)",
    paused: "Paused while ATK HUB is open",
    battery: "battery",
    charging: "charging",
    profile: "Profile",
    auto_switch: "Switch profile by program",
    autostart: "Start with Windows",
    quit: "Quit",
    low_battery: "Low battery",
};

const PT_BR: Strings = Strings {
    open_panel: "Abrir painel",
    searching: "Procurando o mouse…",
    disconnected: "Mouse desconectado",
    asleep: "Mouse sem resposta (dormindo?)",
    paused: "Pausado enquanto o ATK HUB está aberto",
    battery: "bateria",
    charging: "carregando",
    profile: "Perfil",
    auto_switch: "Trocar perfil pelo programa",
    autostart: "Iniciar com o Windows",
    quit: "Sair",
    low_battery: "Bateria fraca",
};

pub fn strings() -> &'static Strings {
    const LANG_PORTUGUESE: u16 = 0x16;
    let primary = unsafe { GetUserDefaultUILanguage() } & 0x3FF;
    if primary == LANG_PORTUGUESE {
        &PT_BR
    } else {
        &EN
    }
}
