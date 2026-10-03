//! Per-sensor rules: DPI range and steps, how a DPI value is stored, and lift-off options.

use crate::tables;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sensor {
    Paw3955Master,
    Paw3950Ultra,
    Paw3950,
    Paw3950Dm,
    Paw3395Ultra,
    Paw3395,
    Paw3395Se,
    Paw3315,
    Paw3311,
}

/// DPI steps: each `(up_to, step)` segment applies to values up to `up_to`, inclusive.
pub type Steps = &'static [(u32, u32)];

/// A lift-off setting: the raw EEPROM value and its height in tenths of a millimetre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiftOff {
    pub raw: u8,
    pub tenths_mm: u8,
}

const fn lod(raw: u8, tenths_mm: u8) -> LiftOff {
    LiftOff { raw, tenths_mm }
}

const LOD_ULTRA: &[LiftOff] = &[
    lod(1, 7), lod(2, 8), lod(3, 9), lod(4, 10), lod(5, 11), lod(6, 12),
    lod(7, 13), lod(8, 14), lod(9, 15), lod(10, 16), lod(11, 17),
];
const LOD_3950: &[LiftOff] = &[lod(3, 7), lod(1, 10), lod(2, 20)];
const LOD_BASIC: &[LiftOff] = &[lod(1, 10), lod(2, 20)];

enum Codec {
    /// PAW3950Ultra: 10-DPI steps up to 10000, then `v * 50 + 10050`, doubled above 30000.
    Ultra,
    /// `(v + 1) * 50`, doubled above 30000.
    Linear,
    /// Lookup table, doubled above 10000.
    Table(&'static [u8; 200]),
    /// PAW3955: 16-bit value in its own EEPROM area (see `eeprom`).
    Wide,
}

impl Sensor {
    pub fn name(self) -> &'static str {
        match self {
            Sensor::Paw3955Master => "PAW3955 Master",
            Sensor::Paw3950Ultra => "PAW3950 Ultra",
            Sensor::Paw3950 => "PAW3950",
            Sensor::Paw3950Dm => "PAW3950DM",
            Sensor::Paw3395Ultra => "PAW3395 Ultra",
            Sensor::Paw3395 => "PAW3395",
            Sensor::Paw3395Se => "PAW3395SE",
            Sensor::Paw3315 => "PAW3315",
            Sensor::Paw3311 => "PAW3311",
        }
    }

    pub fn dpi_min(self) -> u32 {
        match self {
            Sensor::Paw3955Master => 1,
            Sensor::Paw3950Ultra => 10,
            Sensor::Paw3950 | Sensor::Paw3950Dm => 50,
            Sensor::Paw3395Ultra | Sensor::Paw3395 => 100,
            Sensor::Paw3395Se | Sensor::Paw3315 | Sensor::Paw3311 => 200,
        }
    }

    pub fn dpi_steps(self) -> Steps {
        match self {
            Sensor::Paw3955Master => &[(40_000, 1), (50_000, 2)],
            Sensor::Paw3950Ultra => &[(10_000, 10), (30_000, 50), (42_000, 100)],
            Sensor::Paw3950 | Sensor::Paw3950Dm => &[(30_000, 50), (36_000, 100)],
            Sensor::Paw3395Ultra | Sensor::Paw3395 => &[(30_000, 50)],
            Sensor::Paw3395Se => &[(10_000, 50), (18_000, 100)],
            Sensor::Paw3315 => &[(16_000, 100)],
            Sensor::Paw3311 => &[(15_000, 100)],
        }
    }

    pub fn dpi_max(self) -> u32 {
        self.dpi_steps().last().map_or(0, |s| s.0)
    }

    pub fn valid_dpi(self, dpi: u32) -> bool {
        dpi >= self.dpi_min()
            && self
                .dpi_steps()
                .iter()
                .find(|&&(up_to, _)| dpi <= up_to)
                .is_some_and(|&(_, step)| dpi.is_multiple_of(step))
    }

    pub fn lift_off_options(self) -> &'static [LiftOff] {
        match self {
            Sensor::Paw3950Ultra | Sensor::Paw3955Master => LOD_ULTRA,
            Sensor::Paw3950 | Sensor::Paw3950Dm => LOD_3950,
            Sensor::Paw3395Ultra | Sensor::Paw3395 | Sensor::Paw3395Se => LOD_BASIC,
            Sensor::Paw3315 | Sensor::Paw3311 => &[],
        }
    }

    fn codec(self) -> Codec {
        match self {
            Sensor::Paw3955Master => Codec::Wide,
            Sensor::Paw3950Ultra => Codec::Ultra,
            Sensor::Paw3950 | Sensor::Paw3950Dm | Sensor::Paw3395Ultra | Sensor::Paw3395 => Codec::Linear,
            Sensor::Paw3315 => Codec::Table(&tables::PAW3315),
            Sensor::Paw3395Se | Sensor::Paw3311 => Codec::Table(&tables::PAW3395SE),
        }
    }

    /// Whether DPI stages use the 6-byte layout at 0x1B00 instead of the classic 4-byte one.
    pub fn wide_stages(self) -> bool {
        matches!(self.codec(), Codec::Wide)
    }

    /// One axis of a classic stage: register byte plus a nibble where bit 0 = double,
    /// bit 1 = high range (Ultra) or double (tables), bits 2-3 = register bits 8-9.
    pub fn decode_axis(self, low: u8, nibble: u8) -> u32 {
        let v = low as u32 | ((nibble as u32 >> 2) & 3) << 8;
        let double = |dpi: u32, bit: u8| if nibble & bit != 0 { dpi * 2 } else { dpi };
        match self.codec() {
            Codec::Ultra => double(if nibble & 2 != 0 { v * 50 + 10_050 } else { (v + 1) * 10 }, 1),
            Codec::Linear | Codec::Wide => double((v + 1) * 50, 1),
            Codec::Table(table) => {
                let dpi = table.iter().position(|&r| r as u32 == v).map_or(self.dpi_max(), |i| (i as u32 + 1) * 50);
                double(dpi, 2)
            }
        }
    }

    /// Inverse of [`Sensor::decode_axis`]; `None` when the value is not valid for this sensor.
    pub fn encode_axis(self, dpi: u32) -> Option<(u8, u8)> {
        if !self.valid_dpi(dpi) {
            return None;
        }
        let pack = |v: u32, flags: u8| Some(((v & 0xFF) as u8, flags | (((v >> 8) & 3) as u8) << 2));
        match self.codec() {
            Codec::Ultra if dpi <= 10_000 => pack(dpi / 10 - 1, 0),
            Codec::Ultra if dpi <= 30_000 => pack((dpi - 10_050) / 50, 2),
            Codec::Ultra => pack((dpi / 2 - 10_050) / 50, 3),
            Codec::Linear if dpi <= 30_000 => pack(dpi / 50 - 1, 0),
            Codec::Linear => pack(dpi / 100 - 1, 1),
            Codec::Table(table) => {
                let (dpi, flag) = if dpi <= 10_000 { (dpi, 0) } else { (dpi / 2, 2) };
                if !dpi.is_multiple_of(50) || !(50..=10_000).contains(&dpi) {
                    return None;
                }
                pack(table[(dpi / 50 - 1) as usize] as u32, flag)
            }
            Codec::Wide => None,
        }
    }

    /// PAW3955 axis: 16-bit value, doubled with the axis flag from 40000 DPI up.
    pub fn encode_wide(dpi: u32) -> (u16, bool) {
        if dpi < 40_000 {
            ((dpi - 1) as u16, false)
        } else {
            ((dpi / 2 - 1).min(0xFFFF) as u16, true)
        }
    }

    pub fn decode_wide(v: u16, double: bool) -> u32 {
        (v as u32 + 1) * if double { 2 } else { 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Sensor; 9] = [
        Sensor::Paw3955Master,
        Sensor::Paw3950Ultra,
        Sensor::Paw3950,
        Sensor::Paw3950Dm,
        Sensor::Paw3395Ultra,
        Sensor::Paw3395,
        Sensor::Paw3395Se,
        Sensor::Paw3315,
        Sensor::Paw3311,
    ];

    #[test]
    fn every_valid_dpi_round_trips() {
        for s in ALL {
            for dpi in (s.dpi_min()..=s.dpi_max()).filter(|&d| s.valid_dpi(d)) {
                if s.wide_stages() {
                    let (v, d) = Sensor::encode_wide(dpi);
                    assert_eq!(Sensor::decode_wide(v, d), dpi, "{s:?} {dpi}");
                } else {
                    let (low, nibble) = s.encode_axis(dpi).unwrap_or_else(|| panic!("{s:?} {dpi}"));
                    assert_eq!(s.decode_axis(low, nibble), dpi, "{s:?} {dpi}");
                }
            }
        }
    }

    #[test]
    fn steps_match_atk_hub() {
        let s = Sensor::Paw3950Ultra;
        assert!(s.valid_dpi(10) && s.valid_dpi(10_000) && s.valid_dpi(10_050) && s.valid_dpi(42_000));
        assert!(!s.valid_dpi(10_010) && !s.valid_dpi(30_050) && !s.valid_dpi(42_100) && !s.valid_dpi(5));
        assert!(Sensor::Paw3395Se.valid_dpi(18_000) && !Sensor::Paw3395Se.valid_dpi(10_050));
        assert!(Sensor::Paw3950.valid_dpi(36_000) && !Sensor::Paw3950.valid_dpi(30_050));
    }
}
