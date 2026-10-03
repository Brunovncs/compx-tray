//! Active-profile settings stored in the mouse EEPROM (get 0x08 / set 0x07).
//!
//! Every value is followed by a check byte `0x55 - value`; a classic DPI stage `[x, y, ex, crc]`
//! has one crc over its three bytes. Addresses refer to the active profile: to edit another one
//! it has to be activated first.

use crate::info::polling_options;
use crate::sensor::Sensor;
use crate::settings::{sensor_mode, Dpi, MouseSettings, SettingChange, DEBOUNCE_MS, MAX_STAGES};
use crate::{cmd, packet_at, Error, Mouse, Result, PAYLOAD};

/// Polling rate, stage count, current stage, bhop, button mode.
const ADDR_MAIN: u16 = 0x00;
const ADDR_LIFT_OFF: u16 = 0x0A;
/// Classic DPI stages: 4 bytes each, read and written two at a time.
const ADDR_DPI: u16 = 0x0C;
/// PAW3955 DPI stages: 6 bytes each.
const ADDR_WIDE_DPI: u16 = 0x1B00;
/// Debounce, motion sync, LED-off time, angle snapping, ripple control.
const ADDR_FUNCS: u16 = 0xA9;
/// Sensor sleep on/off, sleep time, performance mode.
const ADDR_SENSOR: u16 = 0xB5;

const UNREADABLE: Dpi = Dpi { x: 0, y: 0 };

fn crc(v: u8) -> u8 {
    0x55u8.wrapping_sub(v)
}

/// Reads a `[value, crc]` pair; `None` if the crc does not match.
fn checked(pair: &[u8]) -> Option<u8> {
    (crc(pair[0]) == pair[1]).then_some(pair[0])
}

fn pair(v: u8) -> [u8; 2] {
    [v, crc(v)]
}

fn sum_crc(bytes: &[u8]) -> u8 {
    crc(bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b)))
}

fn polling_from_byte(b: u8) -> Option<u16> {
    Some(match b {
        0x08 => 125,
        0x04 => 250,
        0x02 => 500,
        0x01 => 1000,
        0x10 => 2000,
        0x20 => 4000,
        0x40 => 8000,
        _ => return None,
    })
}

fn polling_to_byte(hz: u16) -> Option<u8> {
    [0x08, 0x04, 0x02, 0x01, 0x10, 0x20, 0x40].into_iter().find(|&b| polling_from_byte(b) == Some(hz))
}

fn stage_decode(sensor: Sensor, b: &[u8]) -> Option<Dpi> {
    let [x, y, ex, c] = [b[0], b[1], b[2], b[3]];
    (sum_crc(&b[..3]) == c).then(|| Dpi { x: sensor.decode_axis(x, ex & 0x0F), y: sensor.decode_axis(y, ex >> 4) })
}

fn stage_encode(sensor: Sensor, dpi: u32) -> Option<[u8; 4]> {
    let (v, nibble) = sensor.encode_axis(dpi)?;
    let ex = nibble | nibble << 4;
    Some([v, v, ex, sum_crc(&[v, v, ex])])
}

fn wide_decode(b: &[u8]) -> Option<Dpi> {
    (sum_crc(&b[..5]) == b[5]).then(|| Dpi {
        x: Sensor::decode_wide(u16::from_le_bytes([b[0], b[1]]), b[4] & 0x01 != 0),
        y: Sensor::decode_wide(u16::from_le_bytes([b[2], b[3]]), b[4] & 0x10 != 0),
    })
}

fn wide_encode(dpi: u32) -> [u8; 6] {
    let (v, double) = Sensor::encode_wide(dpi);
    let [lo, hi] = v.to_le_bytes();
    let flags = if double { 0x11 } else { 0 };
    let mut b = [lo, hi, lo, hi, flags, 0];
    b[5] = sum_crc(&b[..5]);
    b
}

impl Mouse {
    fn read_eeprom<const N: usize>(&self, addr: u16) -> Result<[u8; N]> {
        let reply = self.request(&packet_at(cmd::GET_EEPROM, addr, N as u8, &[]))?;
        Ok(reply[PAYLOAD..PAYLOAD + N].try_into().expect("N <= MAX_PAYLOAD"))
    }

    /// Writes a block and reads it back to confirm.
    fn write_eeprom<const N: usize>(&self, addr: u16, block: [u8; N]) -> Result<()> {
        self.request(&packet_at(cmd::SET_EEPROM, addr, N as u8, &block))?;
        if self.read_eeprom::<N>(addr)? != block {
            return Err(Error::Verify);
        }
        Ok(())
    }

    /// Rewrites a block changing only some bytes.
    fn patch<const N: usize>(&self, addr: u16, edit: impl FnOnce(&mut [u8; N])) -> Result<()> {
        let mut block = self.read_eeprom::<N>(addr)?;
        edit(&mut block);
        self.write_eeprom(addr, block)
    }

    /// Replaces the `[value, crc]` pair at `index` (in pairs) of an `N`-byte block.
    fn set_pair<const N: usize>(&self, addr: u16, index: usize, v: u8) -> Result<()> {
        self.patch::<N>(addr, |b| b[index * 2..index * 2 + 2].copy_from_slice(&pair(v)))
    }

    fn sensor(&self) -> Option<Sensor> {
        self.identity.as_ref()?.sensor()
    }

    /// A stage with a bad crc reads as 0 DPI instead of failing the whole read: stages past the
    /// old count may hold garbage right after the count grows.
    fn read_stages(&self, sensor: Sensor, count: u8) -> Result<Vec<Dpi>> {
        let mut stages = Vec::with_capacity(count as usize);
        if sensor.wide_stages() {
            for i in 0..count as u16 {
                let b = self.read_eeprom::<6>(ADDR_WIDE_DPI + i * 6)?;
                stages.push(wide_decode(&b).unwrap_or(UNREADABLE));
            }
        } else {
            for block in 0..(count as u16).div_ceil(2) {
                let b = self.read_eeprom::<8>(ADDR_DPI + block * 8)?;
                for chunk in b.chunks(4).take(count as usize - stages.len()) {
                    stages.push(stage_decode(sensor, chunk).unwrap_or(UNREADABLE));
                }
            }
        }
        Ok(stages)
    }

    pub fn settings(&self) -> Result<MouseSettings> {
        let main = self.read_eeprom::<10>(ADDR_MAIN)?;
        let stage_count = checked(&main[2..4]).ok_or(Error::Verify)?.clamp(1, MAX_STAGES);
        let lift_off = self.read_eeprom::<2>(ADDR_LIFT_OFF)?;
        let funcs = self.read_eeprom::<10>(ADDR_FUNCS)?;
        let sensor_block = self.read_eeprom::<6>(ADDR_SENSOR)?;
        let flag = |i: usize| checked(&funcs[i * 2..i * 2 + 2]).map(|v| v != 0);

        let identity = self.identity.as_ref();
        let sensor = self.sensor();
        Ok(MouseSettings {
            polling_hz: checked(&main[0..2]).and_then(polling_from_byte),
            polling_options: polling_options(identity.and_then(|i| i.connection)),
            stages: match sensor {
                Some(s) => self.read_stages(s, stage_count)?,
                None => Vec::new(),
            },
            stage_count,
            current_stage: checked(&main[4..6]).unwrap_or(0).min(stage_count - 1),
            dpi_min: sensor.map_or(0, Sensor::dpi_min),
            dpi_steps: sensor.map_or_else(Vec::new, |s| s.dpi_steps().to_vec()),
            lift_off: checked(&lift_off),
            lift_off_options: sensor.map_or(&[][..], Sensor::lift_off_options).iter().map(|&l| l.into()).collect(),
            debounce_ms: checked(&funcs[0..2]),
            motion_sync: flag(1),
            angle_snap: flag(3),
            ripple: flag(4),
            sensor_mode: checked(&sensor_block[4..6]),
            max_sensor_mode: if identity.is_some_and(|i| i.cid == 1) {
                sensor_mode::COMPETITIVE_MAX
            } else {
                sensor_mode::COMPETITIVE
            },
        })
    }

    fn stage_count(&self) -> Result<u8> {
        let main = self.read_eeprom::<10>(ADDR_MAIN)?;
        checked(&main[2..4]).ok_or(Error::Verify)
    }

    pub fn apply(&self, change: SettingChange) -> Result<()> {
        match change {
            SettingChange::Polling { hz } => {
                let allowed = polling_options(self.identity.as_ref().and_then(|i| i.connection));
                let b = polling_to_byte(hz).filter(|_| allowed.contains(&hz)).ok_or(Error::Invalid)?;
                self.set_pair::<10>(ADDR_MAIN, 0, b)
            }
            SettingChange::StageCount { count } => {
                if !(1..=MAX_STAGES).contains(&count) {
                    return Err(Error::Invalid);
                }
                self.patch::<10>(ADDR_MAIN, |m| {
                    m[2..4].copy_from_slice(&pair(count));
                    if checked(&m[4..6]).is_none_or(|cur| cur >= count) {
                        m[4..6].copy_from_slice(&pair(count - 1));
                    }
                })
            }
            SettingChange::CurrentStage { index } => {
                if index >= self.stage_count()? {
                    return Err(Error::Invalid);
                }
                self.set_pair::<10>(ADDR_MAIN, 2, index)
            }
            SettingChange::StageDpi { index, dpi } => {
                let sensor = self.sensor().ok_or(Error::Unsupported)?;
                if index >= MAX_STAGES {
                    return Err(Error::Invalid);
                }
                if sensor.wide_stages() {
                    if !sensor.valid_dpi(dpi) {
                        return Err(Error::Invalid);
                    }
                    self.write_eeprom(ADDR_WIDE_DPI + index as u16 * 6, wide_encode(dpi))
                } else {
                    let stage = stage_encode(sensor, dpi).ok_or(Error::Invalid)?;
                    let at = (index as usize % 2) * 4;
                    self.patch::<8>(ADDR_DPI + (index as u16 / 2) * 8, |b| b[at..at + 4].copy_from_slice(&stage))
                }
            }
            SettingChange::LiftOff { raw } => {
                let sensor = self.sensor().ok_or(Error::Unsupported)?;
                if !sensor.lift_off_options().iter().any(|l| l.raw == raw) {
                    return Err(Error::Invalid);
                }
                self.write_eeprom(ADDR_LIFT_OFF, pair(raw))
            }
            SettingChange::Debounce { ms } => {
                if !DEBOUNCE_MS.contains(&ms) {
                    return Err(Error::Invalid);
                }
                self.set_pair::<10>(ADDR_FUNCS, 0, ms)
            }
            SettingChange::MotionSync { on } => self.set_pair::<10>(ADDR_FUNCS, 1, on as u8),
            SettingChange::AngleSnap { on } => self.set_pair::<10>(ADDR_FUNCS, 3, on as u8),
            SettingChange::Ripple { on } => self.set_pair::<10>(ADDR_FUNCS, 4, on as u8),
            SettingChange::SensorMode { mode } => {
                let max = if self.identity.as_ref().is_some_and(|i| i.cid == 1) {
                    sensor_mode::COMPETITIVE_MAX
                } else {
                    sensor_mode::COMPETITIVE
                };
                if mode > max {
                    return Err(Error::Invalid);
                }
                self.set_pair::<6>(ADDR_SENSOR, 2, mode)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Derived from ATK HUB: [x, y, ex, crc] with X = Y on a PAW3950 Ultra.
    const STAGES: [(u32, [u8; 4]); 8] = [
        (400, [0x27, 0x27, 0x00, 0x07]),
        (800, [0x4F, 0x4F, 0x00, 0xB7]),
        (1600, [0x9F, 0x9F, 0x00, 0x17]),
        (3200, [0x3F, 0x3F, 0x44, 0x93]),
        (10_000, [0xE7, 0xE7, 0xCC, 0xBB]),
        (20_000, [0xC7, 0xC7, 0x22, 0xA5]),
        (30_000, [0x8F, 0x8F, 0x66, 0xD1]),
        (42_000, [0xDB, 0xDB, 0x33, 0x6C]),
    ];

    #[test]
    fn ultra_stages_match_atk_hub() {
        let s = Sensor::Paw3950Ultra;
        for (dpi, bytes) in STAGES {
            assert_eq!(stage_encode(s, dpi), Some(bytes), "encode {dpi}");
            assert_eq!(stage_decode(s, &bytes), Some(Dpi { x: dpi, y: dpi }), "decode {dpi}");
        }
    }

    #[test]
    fn wide_stage_round_trips() {
        for dpi in [1, 800, 39_999, 40_000, 50_000] {
            assert_eq!(wide_decode(&wide_encode(dpi)), Some(Dpi { x: dpi, y: dpi }));
        }
    }

    #[test]
    fn bad_crc_is_rejected() {
        assert_eq!(stage_decode(Sensor::Paw3950Ultra, &[0x4F, 0x4F, 0x00, 0x00]), None);
        assert_eq!(checked(&[0x01, 0x00]), None);
        assert_eq!(checked(&[0x01, 0x54]), Some(1));
    }

    #[test]
    fn polling_round_trips() {
        for hz in crate::info::POLLING_HZ {
            assert_eq!(polling_from_byte(polling_to_byte(hz).unwrap()), Some(hz));
        }
        assert_eq!(polling_to_byte(333), None);
    }
}
