//! What the mouse reports about itself: battery, link type and identity.

use serde::{Deserialize, Serialize};

use crate::catalog;
use crate::sensor::Sensor;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
}

/// Link reported by the handshake (cmd 0x01); decides which polling rates exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Connection {
    Dongle1k,
    Dongle2k,
    Dongle4k,
    Dongle8k,
    Wired1k,
    Wired2k,
    Wired4k,
    Wired8k,
    Nearlink,
}

pub const POLLING_HZ: [u16; 7] = [125, 250, 500, 1000, 2000, 4000, 8000];

impl Connection {
    pub fn from_byte(b: u8) -> Option<Self> {
        Some(match b {
            0 => Connection::Dongle1k,
            1 => Connection::Dongle4k,
            2 => Connection::Wired1k,
            3 => Connection::Wired8k,
            4 => Connection::Dongle2k,
            5 => Connection::Dongle8k,
            6 => Connection::Wired2k,
            7 => Connection::Wired4k,
            8 => Connection::Nearlink,
            _ => return None,
        })
    }

    pub fn is_wired(self) -> bool {
        matches!(self, Connection::Wired1k | Connection::Wired2k | Connection::Wired4k | Connection::Wired8k)
    }

    pub fn max_polling_hz(self) -> u16 {
        match self {
            Connection::Dongle1k | Connection::Wired1k => 1000,
            Connection::Dongle2k | Connection::Wired2k => 2000,
            Connection::Dongle4k | Connection::Wired4k => 4000,
            Connection::Dongle8k | Connection::Wired8k | Connection::Nearlink => 8000,
        }
    }
}

/// Polling rates offered for a link; all of them when the link is unknown.
pub fn polling_options(connection: Option<Connection>) -> Vec<u16> {
    let max = connection.map_or(8000, Connection::max_polling_hz);
    POLLING_HZ.into_iter().filter(|&hz| hz <= max).collect()
}

/// Identity read once per connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    /// `(vendor id, product id)` of the opened device: the mouse itself or its receiver.
    pub usb: (u16, u16),
    pub cid: u8,
    pub mid: u8,
    pub connection: Option<Connection>,
    pub firmware: Option<String>,
    pub receiver_firmware: Option<String>,
}

impl Identity {
    pub fn model(&self) -> Option<&'static catalog::Model> {
        catalog::model(self.cid, self.mid)
    }

    pub fn sensor(&self) -> Option<Sensor> {
        self.model().map(|m| m.sensor)
    }

    pub fn info(&self) -> DeviceInfo {
        let model = self.model();
        DeviceInfo {
            model: model.map_or_else(|| format!("COMPX mouse ({}, {})", self.cid, self.mid), |m| m.name.to_owned()),
            sensor: model.map(|m| m.sensor.name().to_owned()),
            connection: self.connection,
            firmware: self.firmware.clone(),
            receiver_firmware: self.receiver_firmware.clone(),
            profiles: crate::rules::MAX_PROFILES,
            firmware_urls: self.firmware_urls(),
        }
    }

    /// ATK HUB's update service answers `{"data": {"version": "3.03", ...}}` for these.
    fn firmware_urls(&self) -> FirmwareUrls {
        let url = |(vid, pid): (u16, u16), family: &str| format!("{FIRMWARE_SERVICE}/{vid}/{pid}/{family}");
        let model = self.model().filter(|m| m.usb != (0, 0));
        let wireless = self.connection.is_some_and(|c| !c.is_wired());
        FirmwareUrls {
            mouse: model.and_then(|m| Some(url(m.usb, m.firmware?))),
            receiver: (wireless && model.is_none_or(|m| m.usb != self.usb)).then(|| url(self.usb, "dongle")),
        }
    }
}

const FIRMWARE_SERVICE: &str = "https://api-hub-cloud.atk.pro/firmware";

/// Where to ask for the latest firmware of the mouse and of its receiver.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareUrls {
    pub mouse: Option<String>,
    pub receiver: Option<String>,
}

/// Identity in display form, for the panel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub model: String,
    pub sensor: Option<String>,
    pub connection: Option<Connection>,
    pub firmware: Option<String>,
    pub receiver_firmware: Option<String>,
    /// Onboard profiles; ATK HUB uses 4 for every COMPX mouse.
    pub profiles: u8,
    pub firmware_urls: FirmwareUrls,
}
