//! Diagnostics: lists the mouse's HID collections and sends one command to each vendor collection.
//! Usage: `cargo run --example probe -- [cmd_hex] [len] [payload_hex...]` (default: 0x0E, active profile)

use std::time::{Duration, Instant};

use compx_proto::{catalog, cmd, packet_at};
use hidapi::HidApi;

fn hex(s: &str) -> Result<u8, std::num::ParseIntError> {
    u8::from_str_radix(s.trim_start_matches("0x"), 16)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let command = args.next().map(|s| hex(&s)).transpose()?.unwrap_or(cmd::GET_PROFILE);
    let len = args.next().map(|s| s.parse()).transpose()?.unwrap_or(0);
    let payload = args.map(|s| hex(&s)).collect::<Result<Vec<_>, _>>()?;
    let pkt = packet_at(command, 0, len, &payload);
    println!("packet: {pkt:02X?}");

    let api = HidApi::new()?;
    for d in api.device_list().filter(|d| catalog::is_supported(d.vendor_id(), d.product_id())) {
        println!(
            "\nPID {:04X} if {} page {:04X} usage {:04X} {:?}",
            d.product_id(),
            d.interface_number(),
            d.usage_page(),
            d.usage(),
            d.product_string()
        );
        if d.usage_page() < 0xFF00 {
            continue;
        }
        let dev = match d.open_device(&api) {
            Ok(dev) => dev,
            Err(e) => {
                println!("  open failed: {e}");
                continue;
            }
        };
        if let Err(e) = dev.write(&pkt) {
            println!("  write failed: {e}");
            continue;
        }
        let deadline = Instant::now() + Duration::from_millis(1500);
        let mut buf = [0u8; 64];
        while Instant::now() < deadline {
            let n = dev.read_timeout(&mut buf, 100)?;
            if n > 0 {
                println!("  reply ({n} bytes): {:02X?}", &buf[..n]);
            }
        }
    }
    Ok(())
}
