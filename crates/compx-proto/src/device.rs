//! The open HID handle: discovery, request/reply and the simple commands.

use std::cell::Cell;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use crate::info::{Battery, Connection, Identity};
use crate::{catalog, cmd, packet, packet_at, Error, Packet, Result, PAYLOAD, REPORT_ID, REPORT_LEN};

const REPLY_TIMEOUT: Duration = Duration::from_millis(1000);
/// A sleeping mouse tends to drop the first packets; ATK HUB retries too.
const ATTEMPTS: usize = 3;
/// Status-report bit meaning the onboard profile changed.
const STATUS_PROFILE_CHANGED: u8 = 0x04;

/// Unsolicited report from the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    ProfileChanged,
}

pub struct Mouse {
    dev: HidDevice,
    usb: (u16, u16),
    pub identity: Option<Identity>,
    /// A profile-change report was drained while clearing stale replies.
    missed_event: Cell<bool>,
}

fn is_profile_report(buf: &[u8], n: usize) -> bool {
    n > PAYLOAD && buf[0] == REPORT_ID && buf[1] == cmd::STATUS_REPORT && buf[PAYLOAD] & STATUS_PROFILE_CHANGED != 0
}

impl Mouse {
    /// Opens the first supported mouse or receiver. Each HID collection is a separate handle on
    /// Windows and only one accepts output report 0x08 (0xFF02 on the 8K dongle, even though ATK HUB
    /// filters on 0xFF04), so every vendor collection is tried with a harmless read.
    pub fn open(api: &mut HidApi) -> Result<Self> {
        api.refresh_devices()?;
        let probe = packet(cmd::GET_PROFILE, &[]);
        let mut candidates: Vec<_> = api
            .device_list()
            .filter(|d| catalog::is_supported(d.vendor_id(), d.product_id()) && d.usage_page() >= 0xFF00)
            .collect();
        candidates.sort_by_key(|d| d.usage_page() != 0xFF02);
        for info in candidates {
            let Ok(dev) = info.open_device(api) else { continue };
            if dev.write(&probe).is_ok() {
                return Ok(Self { dev, usb: (info.vendor_id(), info.product_id()), identity: None, missed_event: Cell::new(false) });
            }
        }
        Err(Error::NotFound)
    }

    /// Sends a command and waits for the reply echoing it, retrying on timeout.
    pub fn request(&self, pkt: &Packet) -> Result<Packet> {
        for _ in 0..ATTEMPTS {
            match self.request_once(pkt, REPLY_TIMEOUT) {
                Err(Error::Timeout) => continue,
                other => return other,
            }
        }
        Err(Error::Timeout)
    }

    /// Drops replies that arrived after their request timed out, so they cannot be taken as
    /// the answer to the next request.
    fn drain(&self) -> Result<()> {
        let mut buf = [0u8; REPORT_LEN];
        loop {
            let n = self.dev.read_timeout(&mut buf, 0)?;
            if n == 0 {
                return Ok(());
            }
            if is_profile_report(&buf, n) {
                self.missed_event.set(true);
            }
        }
    }

    fn request_once(&self, pkt: &Packet, timeout: Duration) -> Result<Packet> {
        self.drain()?;
        self.dev.write(pkt)?;
        // EEPROM reads echo the address; a reply for another block must not be accepted.
        let eeprom = pkt[1] == cmd::GET_EEPROM;
        let deadline = Instant::now() + timeout;
        let mut buf = [0u8; REPORT_LEN];
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let n = self.dev.read_timeout(&mut buf, left.as_millis().max(1) as i32)?;
            if n >= 5 && buf[0] == REPORT_ID && buf[1] == pkt[1] && (!eeprom || buf[3..5] == pkt[3..5]) {
                return Ok(buf);
            }
            if is_profile_report(&buf, n) {
                self.missed_event.set(true);
            }
        }
        Err(Error::Timeout)
    }

    fn query(&self, command: u8) -> Result<Packet> {
        self.request(&packet(command, &[]))
    }

    /// Active onboard profile, 0-based.
    pub fn profile(&self) -> Result<u8> {
        Ok(self.query(cmd::GET_PROFILE)?[PAYLOAD])
    }

    /// Single short attempt, to check whether a sleeping mouse woke up without blocking for seconds.
    pub fn profile_quick(&self) -> Result<u8> {
        Ok(self.request_once(&packet(cmd::GET_PROFILE, &[]), Duration::from_millis(400))?[PAYLOAD])
    }

    /// The mouse does not always answer this command, so it is sent once, given a short grace
    /// period, and confirmed by reading the profile back.
    pub fn set_profile(&self, index: u8) -> Result<()> {
        match self.request_once(&packet(cmd::SET_PROFILE, &[index]), Duration::from_millis(150)) {
            Ok(_) | Err(Error::Timeout) => {}
            Err(e) => return Err(e),
        }
        if self.profile()? != index {
            return Err(Error::Verify);
        }
        Ok(())
    }

    pub fn battery(&self) -> Result<Battery> {
        let r = self.query(cmd::BATTERY)?;
        Ok(Battery { percent: r[PAYLOAD].min(100), charging: r[PAYLOAD + 1] != 0 })
    }

    pub fn is_online(&self) -> Result<bool> {
        Ok(self.query(cmd::MOUSE_ONLINE)?[PAYLOAD] == 1)
    }

    fn version(&self, command: u8) -> Option<String> {
        let r = self.query(command).ok()?;
        Some(format!("{:x}.{:02x}", r[PAYLOAD], r[PAYLOAD + 1]))
    }

    /// Reads model, link and firmware. Only the CID/MID is required; the rest is best effort.
    pub fn identify(&mut self) -> Result<&Identity> {
        let ids = self.query(cmd::GET_CID_MID)?;
        // ATK HUB sends 4 random bytes it never checks; zeros work the same.
        let connection = self
            .request(&packet_at(cmd::HANDSHAKE, 0, 8, &[0; 4]))
            .ok()
            .and_then(|r| Connection::from_byte(r[PAYLOAD + 6]));
        let identity = Identity {
            usb: self.usb,
            cid: ids[PAYLOAD],
            mid: ids[PAYLOAD + 1],
            connection,
            firmware: self.version(cmd::MOUSE_VERSION),
            receiver_firmware: self.version(cmd::RECEIVER_VERSION),
        };
        Ok(self.identity.insert(identity))
    }

    /// Waits up to `timeout_ms` for an unsolicited report.
    pub fn poll_event(&self, timeout_ms: i32) -> Result<Option<Event>> {
        if self.missed_event.take() {
            return Ok(Some(Event::ProfileChanged));
        }
        let mut buf = [0u8; REPORT_LEN];
        let n = self.dev.read_timeout(&mut buf, timeout_ms)?;
        Ok(is_profile_report(&buf, n).then_some(Event::ProfileChanged))
    }
}
