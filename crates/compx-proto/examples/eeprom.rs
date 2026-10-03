//! Read-only diagnostics: identity, decoded settings and raw EEPROM blocks of the active profile.

use compx_proto::{cmd, packet_at, Mouse};

const BLOCKS: &[(u16, u8, &str)] = &[
    (0x00, 10, "polling / stage count / current stage / bhop / buttons"),
    (0x0A, 2, "lift-off"),
    (0x0C, 8, "DPI 1-2"),
    (0x14, 8, "DPI 3-4"),
    (0x1C, 8, "DPI 5-6"),
    (0x24, 8, "DPI 7-8"),
    (0xA9, 10, "debounce / motion sync / LED off / angle snap / ripple"),
    (0xB5, 6, "sensor sleep / sleep time / sensor mode"),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut api = hidapi::HidApi::new()?;
    let mut mouse = Mouse::open(&mut api)?;
    println!("identity: {:?}", mouse.identify()?);
    println!("info: {:?}", mouse.identity.as_ref().map(|i| i.info()));
    println!("profile: {}", mouse.profile()? + 1);
    println!("battery: {:?}", mouse.battery()?);
    println!("settings: {:#?}", mouse.settings()?);
    for &(addr, len, what) in BLOCKS {
        match mouse.request(&packet_at(cmd::GET_EEPROM, addr, len, &[])) {
            Ok(r) => println!("{addr:#06X} len {len:2}: {:02X?}  ({what})", &r[1..16]),
            Err(e) => println!("{addr:#06X}: {e}"),
        }
    }
    Ok(())
}
