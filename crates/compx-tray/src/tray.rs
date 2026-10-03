//! Tray icon and its menu.

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use compx_proto::ipc::{MouseState, Prefs, Status};
use compx_proto::rules::MAX_PROFILES;

use crate::i18n::Strings;

/// Pastel accent per profile, shared with the panel's CSS and the OSD.
pub const PROFILE_COLORS: [[u8; 3]; MAX_PROFILES as usize] =
    [[0x8E, 0xD8, 0xB0], [0xB9, 0xA7, 0xF5], [0xF7, 0xB2, 0x8C], [0x8C, 0xC8, 0xF7]];
const GRAY: [u8; 3] = [0x9A, 0x9E, 0xA8];
const AMBER: [u8; 3] = [0xF2, 0xD0, 0x7A];

/// Mouse mark in white, 32x32 RGBA (drawn for this project).
const MARK: &[u8; 32 * 32 * 4] = include_bytes!("../assets/mark32.rgba");

pub fn profile_label(t: &Strings, prefs: &Prefs, index: u8) -> String {
    match prefs.profile_names.get(index as usize).map(|n| n.trim()) {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => format!("{} {}", t.profile, index + 1),
    }
}

pub enum MenuAction {
    OpenPanel,
    Profile(u8),
    AutoSwitch(bool),
    Autostart(bool),
    Quit,
}

pub struct Tray {
    icon: TrayIcon,
    open_panel: MenuItem,
    status: MenuItem,
    profiles: Vec<CheckMenuItem>,
    auto_switch: CheckMenuItem,
    autostart: CheckMenuItem,
    quit: MenuItem,
    light_taskbar: bool,
    icons: Vec<([u8; 3], Icon)>,
}

impl Tray {
    pub fn new(t: &Strings, prefs: &Prefs, has_panel: bool) -> Self {
        let open_panel = MenuItem::new(t.open_panel, has_panel, None);
        let status = MenuItem::new(t.searching, false, None);
        let profiles: Vec<_> = (0..MAX_PROFILES)
            .map(|i| CheckMenuItem::new(profile_label(t, prefs, i), false, false, None))
            .collect();
        let auto_switch = CheckMenuItem::new(t.auto_switch, true, prefs.auto_switch, None);
        let autostart = CheckMenuItem::new(t.autostart, true, prefs.autostart, None);
        let quit = MenuItem::new(t.quit, true, None);

        let menu = Menu::new();
        let _ = menu.append_items(&[&open_panel, &PredefinedMenuItem::separator(), &status, &PredefinedMenuItem::separator()]);
        for item in &profiles {
            let _ = menu.append(item);
        }
        let _ = menu.append_items(&[
            &PredefinedMenuItem::separator(),
            &auto_switch,
            &autostart,
            &PredefinedMenuItem::separator(),
            &quit,
        ]);

        let light_taskbar = taskbar_is_light();
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("COMPX Tray")
            .with_icon(render_icon(GRAY, light_taskbar))
            .build()
            .expect("could not create the tray icon");

        Self { icon, open_panel, status, profiles, auto_switch, autostart, quit, light_taskbar, icons: Vec::new() }
    }

    pub fn action(&self, event: &MenuEvent) -> Option<MenuAction> {
        let id = event.id();
        if id == self.open_panel.id() {
            Some(MenuAction::OpenPanel)
        } else if id == self.auto_switch.id() {
            Some(MenuAction::AutoSwitch(self.auto_switch.is_checked()))
        } else if id == self.autostart.id() {
            Some(MenuAction::Autostart(self.autostart.is_checked()))
        } else if id == self.quit.id() {
            Some(MenuAction::Quit)
        } else {
            self.profiles.iter().position(|p| id == p.id()).map(|i| MenuAction::Profile(i as u8))
        }
    }

    fn icon(&mut self, color: [u8; 3]) -> Icon {
        if let Some((_, icon)) = self.icons.iter().find(|(c, _)| *c == color) {
            return icon.clone();
        }
        let icon = render_icon(color, self.light_taskbar);
        self.icons.push((color, icon.clone()));
        icon
    }

    /// Re-renders the icon after the Windows theme changed.
    pub fn theme_changed(&mut self, t: &Strings, status: &Status, prefs: &Prefs) {
        let light = taskbar_is_light();
        if light != self.light_taskbar {
            self.light_taskbar = light;
            self.icons.clear();
            self.show(t, status, prefs);
        }
    }

    pub fn show(&mut self, t: &Strings, status: &Status, prefs: &Prefs) {
        let ready = status.mouse == MouseState::Ready;
        let (label, color) = match (status.mouse, status.profile) {
            (MouseState::Disconnected, _) => (t.disconnected.to_owned(), GRAY),
            (MouseState::Paused, _) => (t.paused.to_owned(), GRAY),
            (MouseState::Asleep, _) | (MouseState::Ready, None) => (t.asleep.to_owned(), AMBER),
            (MouseState::Ready, Some(p)) => {
                let battery = status.battery.map_or_else(String::new, |b| {
                    let charging = if b.charging { format!(" ({})", t.charging) } else { String::new() };
                    format!(" · {} {}%{charging}", t.battery, b.percent)
                });
                let color = PROFILE_COLORS.get(p as usize).copied().unwrap_or(GRAY);
                (format!("{}{battery}", profile_label(t, prefs, p)), color)
            }
        };
        self.status.set_text(&label);
        for (i, item) in self.profiles.iter().enumerate() {
            item.set_text(profile_label(t, prefs, i as u8));
            item.set_enabled(ready);
            item.set_checked(ready && status.profile == Some(i as u8));
        }
        self.auto_switch.set_checked(prefs.auto_switch);
        self.autostart.set_checked(prefs.autostart);
        let icon = self.icon(color);
        let _ = self.icon.set_icon(Some(icon));
        let _ = self.icon.set_tooltip(Some(format!("COMPX Tray — {label}")));
    }
}

/// With a light taskbar the white mark would vanish, so it turns dark.
fn taskbar_is_light() -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|k| k.get_value::<u32, _>("SystemUsesLightTheme"))
        .is_ok_and(|v| v == 1)
}

/// Mouse mark with a status dot in the bottom-right corner.
fn render_icon([r, g, b]: [u8; 3], light_taskbar: bool) -> Icon {
    const N: usize = 32;
    const CENTER: f32 = 25.0;
    const RADIUS: f32 = 6.0;
    const GAP: f32 = 2.0;
    let mark = if light_taskbar { [0x1B, 0x1D, 0x22] } else { [0xFF, 0xFF, 0xFF] };
    let mut rgba = MARK.to_vec();
    for y in 0..N {
        for x in 0..N {
            let i = (y * N + x) * 4;
            let d = ((x as f32 - CENTER).powi(2) + (y as f32 - CENTER).powi(2)).sqrt();
            let dot = (RADIUS + 0.5 - d).clamp(0.0, 1.0);
            // Transparent ring around the dot so it does not blend into the mark.
            let keep = (d - RADIUS - GAP + 0.5).clamp(0.0, 1.0);
            let mark_alpha = rgba[i + 3] as f32 / 255.0 * keep;
            let alpha = dot + mark_alpha * (1.0 - dot);
            let mix = |c: u8, m: u8| {
                if alpha == 0.0 {
                    0
                } else {
                    ((c as f32 * dot + m as f32 * mark_alpha * (1.0 - dot)) / alpha) as u8
                }
            };
            rgba[i..i + 4].copy_from_slice(&[mix(r, mark[0]), mix(g, mark[1]), mix(b, mark[2]), (alpha * 255.0) as u8]);
        }
    }
    Icon::from_rgba(rgba, N as u32, N as u32).expect("valid icon")
}
