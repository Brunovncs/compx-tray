//! Local channel with the panel (compx-tray-ui); message types live in `compx_proto::ipc`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
use windows_sys::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};

use compx_proto::ipc::{self, PanelMessage, TrayMessage};

pub const WM_IPC: u32 = WM_APP + 2;

/// A hello is `{"type":"hello","token":"<32 hex digits>"}`. The limit keeps an unauthenticated
/// client from growing the line without end: the read timeout applies per read, not in total.
const HELLO_MAX: u64 = 256;

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
                (&mut reader).take(HELLO_MAX).read_line(&mut line).ok()?;
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

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;
    use std::sync::mpsc;

    use super::*;

    fn connect(port: u16) -> TcpStream {
        let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        stream
    }

    /// The tray hung up (end of stream or reset) instead of waiting for more input.
    fn dropped(mut stream: &TcpStream) -> bool {
        match stream.read(&mut [0u8; 64]) {
            Ok(n) => n == 0,
            Err(e) => !matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
        }
    }

    #[test]
    fn hello_with_the_token_opens_the_channel() {
        let (tx, rx) = mpsc::channel();
        let (port, token, clients) = serve(tx, 0).unwrap();
        let mut stream = connect(port);
        stream.write_all(ipc::line(&PanelMessage::Hello { token }).as_bytes()).unwrap();
        stream.write_all(ipc::line(&PanelMessage::ReadSettings).as_bytes()).unwrap();

        let wait = Duration::from_secs(2);
        assert!(matches!(rx.recv_timeout(wait), Ok(Incoming::Connected)));
        assert!(matches!(rx.recv_timeout(wait), Ok(Incoming::Message(PanelMessage::ReadSettings))));

        clients.send(&TrayMessage::Focus);
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).unwrap();
        assert_eq!(line, ipc::line(&TrayMessage::Focus));
    }

    #[test]
    fn wrong_token_is_dropped() {
        let (tx, rx) = mpsc::channel();
        let (port, _, _) = serve(tx, 0).unwrap();
        let mut stream = connect(port);
        stream.write_all(ipc::line(&PanelMessage::Hello { token: "0".repeat(32) }).as_bytes()).unwrap();
        stream.write_all(ipc::line(&PanelMessage::ReadSettings).as_bytes()).unwrap();
        assert!(dropped(&stream));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn endless_hello_is_dropped() {
        let (tx, _rx) = mpsc::channel();
        let (port, _, _) = serve(tx, 0).unwrap();
        let mut stream = connect(port);
        // The tray may hang up mid-write; only the outcome matters.
        let _ = stream.write_all(&[b'a'; 64 * 1024]);
        assert!(dropped(&stream));
    }
}
