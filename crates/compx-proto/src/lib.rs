//! HID protocol of COMPX-family mice (ATK, VXE, MAD, Dragonfly), as observed from ATK HUB.
//!
//! Packet: report ID 0x08 + 16 bytes `[cmd, status, addr_hi, addr_lo, len, payload x10, checksum]`,
//! checksum = 0x55 - (0x08 + sum of bytes 0..15), mod 256. Replies echo the command byte.
//!
//! The `hid` feature (default) adds [`Mouse`], which talks to the device; without it the crate
//! only has the data types, rules and the tray/panel IPC messages.

pub mod catalog;
pub mod info;
pub mod ipc;
pub mod procs;
pub mod rules;
pub mod sensor;
pub mod settings;
mod tables;

#[cfg(feature = "hid")]
mod device;
#[cfg(feature = "hid")]
mod eeprom;

#[cfg(feature = "hid")]
pub use device::{Event, Mouse};
#[cfg(feature = "hid")]
pub use hidapi::HidError;

pub const REPORT_ID: u8 = 0x08;
pub const REPORT_LEN: usize = 17;
/// Index of the first payload byte in a report (after report ID, cmd, status, addr and len).
pub const PAYLOAD: usize = 6;
pub const MAX_PAYLOAD: usize = 10;

pub mod cmd {
    pub const HANDSHAKE: u8 = 0x01;
    pub const MOUSE_ONLINE: u8 = 0x03;
    pub const BATTERY: u8 = 0x04;
    pub const SET_EEPROM: u8 = 0x07;
    pub const GET_EEPROM: u8 = 0x08;
    pub const STATUS_REPORT: u8 = 0x0A;
    pub const GET_PROFILE: u8 = 0x0E;
    pub const SET_PROFILE: u8 = 0x0F;
    pub const GET_CID_MID: u8 = 0x10;
    pub const MOUSE_VERSION: u8 = 0x12;
    pub const RECEIVER_VERSION: u8 = 0x1D;
}

pub type Packet = [u8; REPORT_LEN];

pub fn packet(command: u8, payload: &[u8]) -> Packet {
    packet_at(command, 0, payload.len() as u8, payload)
}

/// Packet with an EEPROM address; for reads `len` is how many bytes to ask for and the payload is empty.
pub fn packet_at(command: u8, addr: u16, len: u8, payload: &[u8]) -> Packet {
    assert!(payload.len() <= MAX_PAYLOAD && len as usize <= MAX_PAYLOAD);
    let mut buf = [0u8; REPORT_LEN];
    buf[0] = REPORT_ID;
    buf[1] = command;
    buf[3..5].copy_from_slice(&addr.to_be_bytes());
    buf[5] = len;
    buf[PAYLOAD..PAYLOAD + payload.len()].copy_from_slice(payload);
    let sum = buf[..16].iter().fold(0u8, |acc, b| acc.wrapping_add(*b));
    buf[16] = 0x55u8.wrapping_sub(sum);
    buf
}

#[derive(Debug)]
pub enum Error {
    NotFound,
    Timeout,
    /// Value outside what the mouse accepts.
    Invalid,
    /// The mouse model or sensor is unknown, so the setting cannot be encoded safely.
    Unsupported,
    /// Data read back does not check out (crc or read-after-write).
    Verify,
    #[cfg(feature = "hid")]
    Hid(HidError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotFound => write!(f, "mouse not found"),
            Error::Timeout => write!(f, "the mouse did not respond"),
            Error::Invalid => write!(f, "invalid value for this mouse"),
            Error::Unsupported => write!(f, "not supported for this mouse model"),
            Error::Verify => write!(f, "the mouse returned inconsistent data"),
            #[cfg(feature = "hid")]
            Error::Hid(e) => write!(f, "HID error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(feature = "hid")]
impl From<HidError> for Error {
    fn from(e: HidError) -> Self {
        Error::Hid(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_atk_hub() {
        assert_eq!(packet(cmd::SET_PROFILE, &[0])[16], 0x3D);
        assert_eq!(packet(cmd::SET_PROFILE, &[1])[16], 0x3C);
        assert_eq!(packet(cmd::GET_PROFILE, &[])[16], 0x3F);
        assert_eq!(packet(cmd::GET_CID_MID, &[])[16], 0x3D);
    }
}
