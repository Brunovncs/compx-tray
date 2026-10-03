//! COMPX Tray panel. It never touches the mouse: it relays messages to and from the tray
//! (compx-tray.exe), which owns the HID handle. Closing the window ends only this process.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use compx_proto::ipc::{self, PanelMessage, TrayMessage};
use tauri::{AppHandle, Emitter, Manager, State};
use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows_sys::Win32::System::Threading::CreateMutexW;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

const TRAY_EXE: &str = "compx-tray.exe";
const RETRY: Duration = Duration::from_millis(700);

/// Latest message of each kind, so a freshly loaded page can catch up.
#[derive(Default)]
struct Snapshot {
    status: Option<TrayMessage>,
    prefs: Option<TrayMessage>,
    backups: Option<TrayMessage>,
}

#[derive(Default)]
struct Link {
    writer: Mutex<Option<TcpStream>>,
    snapshot: Mutex<Snapshot>,
}

impl Link {
    fn send(&self, msg: &PanelMessage) -> Result<(), String> {
        let mut writer = self.writer.lock().unwrap();
        let stream = writer.as_mut().ok_or("tray-offline")?;
        stream.write_all(ipc::line(msg).as_bytes()).map_err(|e| e.to_string())
    }
}

/// Connects and authenticates with the token the tray left in the registry.
fn connect() -> Option<TcpStream> {
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\compx-tray").ok()?;
    let port: u32 = key.get_value("ipc_port").ok()?;
    let token: String = key.get_value("ipc_token").ok()?;
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port as u16));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(300)).ok()?;
    stream.write_all(ipc::line(&PanelMessage::Hello { token }).as_bytes()).ok()?;
    Some(stream)
}

fn launch_tray() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe.with_file_name(TRAY_EXE)).spawn();
    }
}

fn focus(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn set_connected(app: &AppHandle, connected: bool) {
    let _ = app.emit("link", connected);
}

/// Keeps the connection to the tray alive; on the first failure tries to start it.
fn run_link(app: AppHandle) {
    let link = app.state::<Link>();
    let mut tried_launch = false;
    loop {
        if let Some(stream) = connect() {
            tried_launch = true;
            if let Ok(writer) = stream.try_clone() {
                *link.writer.lock().unwrap() = Some(writer);
                set_connected(&app, true);
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    let Ok(msg) = serde_json::from_str::<TrayMessage>(&line) else { continue };
                    {
                        let mut snap = link.snapshot.lock().unwrap();
                        match msg {
                            TrayMessage::Status { .. } => snap.status = Some(msg.clone()),
                            TrayMessage::Prefs { .. } => snap.prefs = Some(msg.clone()),
                            TrayMessage::Backups { .. } => snap.backups = Some(msg.clone()),
                            TrayMessage::Focus => focus(&app),
                            _ => {}
                        }
                    }
                    let _ = app.emit("tray", msg);
                }
                *link.writer.lock().unwrap() = None;
                *link.snapshot.lock().unwrap() = Snapshot::default();
            }
            set_connected(&app, false);
        } else if !tried_launch {
            tried_launch = true;
            launch_tray();
        }
        thread::sleep(RETRY);
    }
}

#[derive(serde::Serialize)]
struct Initial {
    connected: bool,
    messages: Vec<TrayMessage>,
}

#[tauri::command]
fn initial_state(link: State<Link>) -> Initial {
    let connected = link.writer.lock().unwrap().is_some();
    let snap = link.snapshot.lock().unwrap();
    let messages = [&snap.status, &snap.prefs, &snap.backups].into_iter().flatten().cloned().collect();
    Initial { connected, messages }
}

#[tauri::command]
fn send(link: State<Link>, msg: PanelMessage) -> Result<(), String> {
    if matches!(msg, PanelMessage::Hello { .. }) {
        return Err("not allowed".into());
    }
    link.send(&msg)
}

/// Programs running now, to pick in a rule instead of typing the exe name.
#[tauri::command]
fn running_programs() -> Vec<String> {
    compx_proto::procs::running_exes()
}

#[tauri::command]
fn start_tray() {
    launch_tray();
}

/// Opens ATK HUB (for firmware updates). The tray pauses by itself while it runs.
#[tauri::command]
fn open_atk_hub() -> Result<(), String> {
    let roots = [
        std::env::var_os("LOCALAPPDATA").map(|d| std::path::PathBuf::from(d).join("Programs")),
        std::env::var_os("ProgramFiles").map(Into::into),
    ];
    let exe = roots
        .into_iter()
        .flatten()
        .map(|root| root.join("ATK HUB").join("ATK HUB.exe"))
        .find(|p| p.exists())
        .ok_or("atk-hub-not-found")?;
    std::process::Command::new(exe).spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn already_running() -> bool {
    let name: Vec<u16> = "Local\\compx-tray-ui-single-instance\0".encode_utf16().collect();
    unsafe {
        CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        GetLastError() == ERROR_ALREADY_EXISTS
    }
}

fn main() {
    // No console in release builds: panics go to %TEMP%\compx-tray-ui-crash.txt.
    std::panic::set_hook(Box::new(|info| {
        let _ = std::fs::write(std::env::temp_dir().join("compx-tray-ui-crash.txt"), info.to_string());
    }));

    // Second instance: ask the tray to bring the existing window forward.
    if already_running() {
        if let Some(mut stream) = connect() {
            let _ = stream.write_all(ipc::line(&PanelMessage::Focus).as_bytes());
        }
        return;
    }

    tauri::Builder::default()
        .manage(Link::default())
        .invoke_handler(tauri::generate_handler![initial_state, send, running_programs, start_tray, open_atk_hub])
        .setup(|app| {
            let handle = app.handle().clone();
            thread::spawn(move || run_link(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("could not start the panel");
}
