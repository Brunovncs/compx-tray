//! Local channel with the panel (compx-tray-ui); message types live in `compx_proto::ipc`.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
use windows_sys::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};

use compx_proto::ipc::{self, PanelMessage, TrayMessage};

pub const WM_IPC: u32 = WM_APP + 2;

/// Panel messages as they reach the UI thread. `Connected` asks for the initial state.
pub enum Incoming {
    Connected,
    Message(PanelMessage),
}

#[derive(Clone, Default)]
pub struct Clients(Arc<Mutex<Vec<TcpStream>>>);

impl Clients {
    pub fn send(&self, msg: &TrayMessage) {
        let line = ipc::line(msg);
        let mut clients = self.0.lock().unwrap();
        clients.retain_mut(|c| c.write_all(line.as_bytes()).is_ok());
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 16];
    let status =
        unsafe { BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), bytes.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    // An all-zero token would let any local page talk to the tray.
    assert_eq!(status, 0, "BCryptGenRandom failed: {status:#x}");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Listens on a free port; returns the port and token for the panel to find.
pub fn serve(tx: Sender<Incoming>, ui_thread: u32) -> std::io::Result<(u16, String, Clients)> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let port = listener.local_addr()?.port();
    let token = random_token();
    let clients = Clients::default();
    let registry = clients.clone();
    let expected = token.clone();

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (tx, registry, expected) = (tx.clone(), registry.clone(), expected.clone());
            thread::spawn(move || {
                let post = |msg| {
                    let sent = tx.send(msg).is_ok();
                    unsafe { PostThreadMessageW(ui_thread, WM_IPC, 0, 0) };
                    sent
                };
                // `try_clone` is a separate socket handle on Windows with its own timeouts, so the
                // hello timeout is set and cleared on the handle that actually reads.
                let reader = stream.try_clone().ok()?;
                let _ = reader.set_read_timeout(Some(Duration::from_secs(5)));
                let mut reader = BufReader::new(reader);
                let mut line = String::new();
                reader.read_line(&mut line).ok()?;
                match serde_json::from_str(&line) {
                    Ok(PanelMessage::Hello { token }) if token == expected => {}
                    _ => return None,
                }
                let _ = reader.get_ref().set_read_timeout(None);
                let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
                registry.0.lock().unwrap().push(stream);
                post(Incoming::Connected);
                loop {
                    line.clear();
                    if !reader.read_line(&mut line).is_ok_and(|n| n > 0) {
                        break;
                    }
                    if let Ok(msg) = serde_json::from_str::<PanelMessage>(&line) {
                        if !post(Incoming::Message(msg)) {
                            break;
                        }
                    }
                }
                Some(())
            });
        }
    });

    Ok((port, token, clients))
}
