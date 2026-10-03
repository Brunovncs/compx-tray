#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backups;
mod i18n;
mod ipc;
mod prefs;
mod tray;
mod win;
mod worker;

use std::path::PathBuf;
use std::process::Command as Process;
use std::sync::mpsc::{self, Sender};

use tray_icon::menu::MenuEvent;
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentThreadId};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, PostQuitMessage, TranslateMessage, MSG, PM_NOREMOVE,
    WM_HOTKEY,
};

use compx_proto::ipc::{MouseState, PanelMessage, Prefs, Status, TrayMessage};
use compx_proto::rules::{RuleMode, MAX_PROFILES};

use i18n::Strings;
use ipc::{Clients, Incoming, WM_IPC};
use tray::{MenuAction, Tray, PROFILE_COLORS};
use win::{Win, WM_DEVICES_CHANGED, WM_FOREGROUND_CHANGED, WM_THEME_CHANGED};
use worker::{AutoSwitch, Command, WorkerEvent, WM_WORKER};

const PANEL_EXE: &str = "compx-tray-ui.exe";
const LOW_BATTERY: u8 = 15;
/// Re-arms the low-battery toast once charged past this.
const LOW_BATTERY_RESET: u8 = 25;

/// Central state: tray menu, panel and hotkeys all go through the same methods.
struct App {
    t: &'static Strings,
    tray: Tray,
    win: Win,
    worker: Sender<Command>,
    clients: Clients,
    status: Status,
    prefs: Prefs,
    low_battery_warned: bool,
    /// A backup was requested; the next settings read gets saved.
    backup_pending: bool,
}

impl App {
    fn auto_switch(&self) -> AutoSwitch {
        AutoSwitch {
            enabled: self.prefs.auto_switch,
            mode: self.prefs.rule_mode,
            rules: self.prefs.rules.clone(),
            default_profile: self.prefs.default_profile,
        }
    }

    fn send_foreground(&self) {
        let _ = self.worker.send(Command::Foreground(compx_proto::procs::foreground_exe()));
    }

    /// Applies preferences to the worker, hooks and hotkeys, and tells the panel.
    fn apply_prefs(&mut self) {
        let focused = self.prefs.auto_switch && self.prefs.rule_mode == RuleMode::Focused;
        self.win.set_foreground_hook(focused);
        self.win.set_hotkeys(if self.prefs.hotkeys { MAX_PROFILES } else { 0 });
        let _ = self.worker.send(Command::AutoSwitch(self.auto_switch()));
        if focused {
            self.send_foreground();
        }
        self.tray.show(self.t, &self.status, &self.prefs);
        self.clients.send(&TrayMessage::Prefs { prefs: self.prefs.clone() });
    }

    fn set_prefs(&mut self, mut new: Prefs) {
        new.profile_names.resize(MAX_PROFILES as usize, String::new());
        for name in &mut new.profile_names {
            *name = name.replace(['\n', '\r'], " ").trim().chars().take(32).collect();
        }
        new.default_profile = new.default_profile.min(MAX_PROFILES - 1);
        new.rules.retain(|r| r.profile < MAX_PROFILES);
        if new.autostart != self.prefs.autostart {
            prefs::set_autostart(new.autostart);
            new.autostart = prefs::autostart();
        }
        prefs::save(&new);
        self.prefs = new;
        self.apply_prefs();
    }

    fn set_status(&mut self, status: Status) {
        let ready = status.mouse == MouseState::Ready;
        // Only real switches, not the first reading after connecting.
        if let (true, true, Some(old), Some(new)) = (ready, self.prefs.osd, self.status.profile, status.profile) {
            if old != new {
                self.show_profile_osd(new);
            }
        }
        if let Some(b) = status.battery.filter(|_| ready) {
            if b.charging || b.percent >= LOW_BATTERY_RESET {
                self.low_battery_warned = false;
            } else if b.percent <= LOW_BATTERY && !self.low_battery_warned {
                self.low_battery_warned = true;
                self.win.show_osd(&format!("{} · {}%", self.t.low_battery, b.percent), [0xF4, 0xA4, 0xA4]);
            }
        }
        self.status = status;
        self.tray.show(self.t, &self.status, &self.prefs);
        self.clients.send(&TrayMessage::Status { status: self.status.clone() });
    }

    fn show_profile_osd(&self, profile: u8) {
        let color = PROFILE_COLORS.get(profile as usize).copied().unwrap_or([0xDD; 3]);
        self.win.show_osd(&tray::profile_label(self.t, &self.prefs, profile), color);
    }

    fn set_profile(&self, index: u8) {
        // Also while "asleep": the status lags behind, and the request itself reaches a woken mouse.
        if index < MAX_PROFILES && matches!(self.status.mouse, MouseState::Ready | MouseState::Asleep) {
            let _ = self.worker.send(Command::SetProfile(index));
        }
    }

    fn notice(&self, ok: bool, message: &str) {
        self.clients.send(&TrayMessage::Notice { ok, message: message.to_owned() });
    }

    fn on_worker(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::Status(status) => self.set_status(status),
            WorkerEvent::Settings(Ok(settings)) => {
                if std::mem::take(&mut self.backup_pending) {
                    let model = self.status.device.as_ref().map_or("", |d| d.model.as_str());
                    let saved = backups::save(model, self.status.profile.unwrap_or(0), settings.clone());
                    self.notice(saved.is_ok(), if saved.is_ok() { "backupSaved" } else { "backupFailed" });
                    self.clients.send(&TrayMessage::Backups { backups: backups::list() });
                }
                self.clients.send(&TrayMessage::Settings { settings });
            }
            WorkerEvent::Settings(Err(message)) => {
                if std::mem::take(&mut self.backup_pending) {
                    self.notice(false, "backupFailed");
                }
                self.clients.send(&TrayMessage::SettingsError { message });
            }
            WorkerEvent::Restored(result) => self.notice(result.is_ok(), "restored"),
        }
    }

    fn on_panel(&mut self, msg: Incoming) {
        let msg = match msg {
            Incoming::Connected => {
                self.clients.send(&TrayMessage::Status { status: self.status.clone() });
                self.clients.send(&TrayMessage::Prefs { prefs: self.prefs.clone() });
                self.clients.send(&TrayMessage::Backups { backups: backups::list() });
                return;
            }
            Incoming::Message(msg) => msg,
        };
        match msg {
            PanelMessage::Hello { .. } => {}
            PanelMessage::Focus => self.clients.send(&TrayMessage::Focus),
            PanelMessage::SetProfile { index } => self.set_profile(index),
            PanelMessage::SetPrefs { prefs } => self.set_prefs(prefs),
            PanelMessage::ReadSettings => {
                let _ = self.worker.send(Command::ReadSettings);
            }
            PanelMessage::Write { change } => {
                let _ = self.worker.send(Command::Write(change));
            }
            PanelMessage::Backup => {
                self.backup_pending = true;
                let _ = self.worker.send(Command::ReadSettings);
            }
            PanelMessage::Restore { name } => match backups::load(&name) {
                Some(saved) => {
                    let _ = self.worker.send(Command::Restore(saved));
                }
                None => self.notice(false, "restored"),
            },
            PanelMessage::DeleteBackup { name } => {
                backups::delete(&name);
                self.clients.send(&TrayMessage::Backups { backups: backups::list() });
            }
        }
    }

    /// Returns `false` to quit.
    fn on_menu(&mut self, event: MenuEvent) -> bool {
        match self.tray.action(&event) {
            Some(MenuAction::OpenPanel) => open_panel(),
            Some(MenuAction::Profile(p)) => {
                self.set_profile(p);
                // Undo the check the menu toggled on its own; the real state arrives from the worker.
                self.tray.show(self.t, &self.status, &self.prefs);
            }
            Some(MenuAction::AutoSwitch(on)) => self.set_prefs(Prefs { auto_switch: on, ..self.prefs.clone() }),
            Some(MenuAction::Autostart(on)) => self.set_prefs(Prefs { autostart: on, ..self.prefs.clone() }),
            Some(MenuAction::Quit) => return false,
            None => {}
        }
        true
    }
}

fn panel_path() -> Option<PathBuf> {
    let path = std::env::current_exe().ok()?.with_file_name(PANEL_EXE);
    path.exists().then_some(path)
}

/// If a panel is already open, the new instance asks it to come forward and exits.
fn open_panel() {
    if let Some(path) = panel_path() {
        let _ = Process::new(path).spawn();
    }
}

fn already_running() -> bool {
    let name: Vec<u16> = "Local\\compx-tray-single-instance\0".encode_utf16().collect();
    unsafe {
        CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        GetLastError() == ERROR_ALREADY_EXISTS
    }
}

fn main() {
    // No console in release builds: panics go to %TEMP%\compx-tray-crash.txt.
    std::panic::set_hook(Box::new(|info| {
        let _ = std::fs::write(std::env::temp_dir().join("compx-tray-crash.txt"), info.to_string());
    }));

    if already_running() {
        return;
    }
    prefs::init();

    let ui_thread = unsafe {
        // Make sure the thread's message queue exists before other threads post to it.
        let mut msg: MSG = std::mem::zeroed();
        PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
        GetCurrentThreadId()
    };

    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    let (panel_tx, panel_rx) = mpsc::channel();

    let clients = match ipc::serve(panel_tx, ui_thread) {
        Ok((port, token, clients)) => {
            prefs::set_ipc_endpoint(port, &token);
            clients
        }
        Err(_) => Clients::default(),
    };

    let t = i18n::strings();
    let prefs = prefs::load();
    let mut app = App {
        t,
        tray: Tray::new(t, &prefs, panel_path().is_some()),
        win: Win::new(),
        worker: cmd_tx,
        clients,
        status: Status::DISCONNECTED,
        prefs,
        low_battery_warned: false,
        backup_pending: false,
    };
    worker::spawn(cmd_rx, event_tx, ui_thread, app.auto_switch());
    app.apply_prefs();

    let menu_events = MenuEvent::receiver();
    let tray_events = TrayIconEvent::receiver();
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    'running: while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
        match (msg.hwnd.is_null(), msg.message) {
            (true, WM_WORKER) => {
                while let Ok(event) = event_rx.try_recv() {
                    app.on_worker(event);
                }
            }
            (true, WM_IPC) => {
                while let Ok(msg) = panel_rx.try_recv() {
                    app.on_panel(msg);
                }
            }
            (true, WM_DEVICES_CHANGED) => {
                let _ = app.worker.send(Command::DevicesChanged);
            }
            (true, WM_FOREGROUND_CHANGED) => app.send_foreground(),
            (true, WM_THEME_CHANGED) => app.tray.theme_changed(app.t, &app.status, &app.prefs),
            (true, WM_HOTKEY) => app.set_profile(msg.wParam as u8 - 1),
            _ => unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }

        while let Ok(event) = tray_events.try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                open_panel();
            }
        }
        while let Ok(event) = menu_events.try_recv() {
            if !app.on_menu(event) {
                let _ = app.worker.send(Command::Quit);
                unsafe { PostQuitMessage(0) };
                break 'running;
            }
        }
    }
}
