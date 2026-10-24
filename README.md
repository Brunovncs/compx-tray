# COMPX Tray

A small Windows tray app for gaming mice built on the COMPX controller, sold under the ATK, VXE,
MAD and Dragonfly brands. It covers what the vendor's ATK HUB is used for day to day (switching the
mouse's four onboard profiles, switching them automatically per program, and editing DPI, polling
rate and the other per-profile settings) without keeping a 503 MB Electron app installed and
running. The tray is a 0.7 MB executable with about 2 MB of private memory and no web view. An
optional settings panel runs as a separate process and only exists while its window is open.

COMPX Tray is open source (MIT), independent from the mouse makers, and in development. It is used
daily on one mouse, an ATK Y9 Ultimate; every other model is recognised from the vendor's device
list but has not been tried (see [Supported hardware](#supported-hardware)). Treat it as a beta.

## Installing

There are no prebuilt binaries yet. You need Windows 10 or 11 and Rust; the repository pins the
`stable-x86_64-pc-windows-gnu` toolchain, which rustup installs on first build. The settings panel
uses WebView2, which ships with Windows 11 and current Windows 10.

```powershell
.\install.ps1     # release build, copied to %LOCALAPPDATA%\Programs\compx-tray, then started
.\uninstall.ps1   # removes the program, its preferences, the autostart entry and the backups
```

Neither script needs administrator rights or downloads anything besides the Rust crates Cargo
fetches for the build. On first start the tray adds itself to "Start with Windows" (the per-user
`Run` key); it can be turned off from the tray menu. Settings already written to the mouse stay on
the mouse after uninstalling.

ATK HUB can stay installed. While `ATK HUB.exe` is running the tray releases the mouse and pauses,
so firmware updates and anything else ATK HUB does are left to ATK HUB.

## What it does

The tray owns the mouse. It shows the active profile as a coloured dot on its icon and the battery
level in its tooltip, switches between the four onboard profiles from its menu or with
`Ctrl+Alt+Shift+1` to `4`, shows a short on-screen toast on every switch, and warns at 15 %
battery. It reconnects when the receiver or the mouse is plugged in, driven by `WM_DEVICECHANGE`
instead of polling, and follows the taskbar's light or dark theme.

Profiles can follow programs. A rule ties a program to a profile and applies either while the
program is running, checked every 3 s, or while it has focus, which is event driven through
`SetWinEventHook` and switches immediately. When no rule matches, a default profile is used.

The panel edits what the mouse stores per profile: DPI stages (1 to 8, with the active stage),
polling rate (the options follow the receiver, up to 8 kHz on an 8K dongle), lift-off distance,
click debounce, motion sync, angle snapping, ripple control and sensor mode. It also names
profiles, edits the program rules, saves a profile's settings to a JSON backup and restores it,
shows the mouse and receiver firmware versions, and says when ATK HUB has a newer firmware (see
[Network](#network)). The interface is in English or Brazilian Portuguese, following the Windows
language.

## Supported hardware

The mouse is identified in two steps: the USB vendor and product id says it speaks the COMPX
protocol, and the model and sensor are then read from the mouse itself (its CID and MID). Both
tables come from the device list of ATK HUB 3.2.27.

| | Count | Source |
|---|---|---|
| USB ids recognised (mice and receivers, vendors `0x373B` and `0x3554`) | 179 | ATK HUB 3.2.27 |
| Models (ATK Y9, A9, X1, F1, U2, Z1, ZERO, Duckbill, Ghost; VXE R1, V3, X3; MAD R; Dragonfly) | 154 | ATK HUB 3.2.27 |
| Sensors with a DPI encoding (PAW3955, 3950 Ultra, 3950, 3950DM, 3395 Ultra, 3395, 3395SE, 3315, 3311) | 9 | ATK HUB 3.2.27, unit tests |
| **Tested on hardware**: ATK Y9 Ultimate (PAW3950 Ultra) on the 8K receiver, `373B:11D9` | 1 | daily use |

The full list is in [`catalog.rs`](crates/compx-proto/src/catalog.rs). A model whose CID/MID is not
in the table is still detected, but its DPI is read-only. Reports from other models are welcome,
in particular the output of the read-only `eeprom` diagnostic below.

Not supported: ATK mice built on other controllers with other protocols (BITMOUSE, HOLTEK, HECHI,
EVISION; for example the Y9 Pro and the A9 3.0), and keyboards.

## What it writes to the mouse

Settings are written to the active profile in the mouse's onboard memory (EEPROM), at the
addresses listed in [`docs/protocol.md`](docs/protocol.md) and nowhere else. Every value is checked
against the range of the detected sensor before it is sent, and every write is read back and
compared; a mismatch is reported as an error. COMPX Tray never writes firmware: it only reads the
version numbers, and flashing stays in ATK HUB.

The risk to know about: only the PAW3950 Ultra encoding has been compared with bytes ATK HUB itself
produces and run on a real mouse. The other 8 sensors' encodings follow ATK HUB's tables and pass
round-trip tests, but have never been written to a mouse of that sensor. A wrong encoding would store
a wrong DPI, which the read-back would not catch because it compares bytes, not behaviour. Back up
a profile from the panel before changing DPI on an untested model.

## How it works

```
crates/
  compx-proto/     protocol library: packets, device discovery, EEPROM settings, sensors,
                   model catalog, rules, process list and the tray/panel IPC types
  compx-tray/      tray: Win32 message loop, HID worker thread, IPC server, OSD, hotkeys
  compx-tray-ui/   Tauri 2 panel: a thin relay between the web UI and the tray
```

The tray is a plain Win32 message loop. A worker thread owns the HID handle, so the UI thread never
waits on the mouse, and the program checks run without allocating. The panel never touches the
mouse: it connects to the tray over TCP on `127.0.0.1`, on a random port, and authenticates with a
random 128-bit token; both are stored in the per-user key `HKCU\Software\compx-tray`. The token is there because any web page can send requests to localhost. Messages are JSON
lines, defined once in [`compx_proto::ipc`](crates/compx-proto/src/ipc.rs). Closing the panel ends
its process and frees the web view; the tray keeps running.

ATK HUB is detected by its process, checked every 3 s, so there is a window of up to 3 s after it
starts before the tray steps aside.

### Network

The tray makes no network requests. While the panel is open it asks ATK HUB's public update service
(`api-hub-cloud.atk.pro`) for the latest firmware version of the connected mouse and receiver,
sending their USB ids and firmware family, as ATK HUB does. Nothing else leaves the machine.

## Measurements

| | Value | How it was measured |
|---|---|---|
| Tray executable, `compx-tray.exe` | 742,400 bytes (0.71 MB) | release build, GNU toolchain, LTO, `opt-level = "z"`, stripped |
| Panel executable, `compx-tray-ui.exe` | 3,032,064 bytes (2.9 MB) | same build; the GNU build also needs `WebView2Loader.dll` (160,880 bytes) |
| Tray memory, idle with the mouse connected | 1.8 MB private, 10.7 MB working set | `Get-Process` 20 s after start, private bytes and working set |
| ATK HUB 3.2.27, installed | 503.1 MB | size of `C:\Program Files\ATK HUB` |

The memory figure that matters for a resident tray is private bytes, the memory the process holds
on its own; the working set also counts shared system DLL pages.

## Building and testing

```powershell
cargo build --release
cargo test --workspace
cargo clippy --workspace --all-targets
```

The 14 tests cover the protocol library: packet checksums, PAW3950 Ultra DPI stages and a sample of
DPI steps (3950 Ultra, 3950, 3395SE) compared with values taken from ATK HUB, a round trip of every valid DPI value
of all 9 sensors, the PAW3955 wide encoding, polling rate, rejection of a bad CRC, the model catalog
(sorted, unique, finds the Y9 Ultimate), program rules (including case-insensitive non-ASCII names)
and the IPC messages. The tray has 6 tests: the IPC handshake over a real socket (the right token
opens the channel; a wrong token or an endless hello line is dropped), the backup file-name check,
and restoring lift-off by height, since its raw values differ between sensors. The panel has no automated tests.

Protocol notes are in [`docs/protocol.md`](docs/protocol.md). Two diagnostics talk to a connected
mouse:

```powershell
cargo run -p compx-proto --example eeprom        # read-only: identity, decoded settings, raw EEPROM
cargo run -p compx-proto --example probe -- 0E   # sends the given command and payload to every vendor collection
```

`eeprom` only reads. `probe` sends whatever you give it, including write commands, so use it only
if you know the command.

## Limitations

- Windows only. The tray is Win32 code; the protocol library would port, the tray would not.
- One mouse tested. DPI on the other 8 sensors is unverified on hardware (see above).
- No prebuilt or signed binaries. It is built from source, and an unsigned executable may trigger
  SmartScreen if you copy it elsewhere.
- There is a gap of up to 3 s between ATK HUB starting and the tray pausing.
- ATK HUB is still needed for firmware updates and for mice on other controllers.

## Disclaimer

COMPX Tray is an independent project. It is not affiliated with, endorsed or sponsored by ATK, VXE,
MAD, Dragonfly, COMPX or PixArt; those names are trademarks of their owners and are used only to
identify compatible hardware. No code, firmware or artwork from ATK HUB is included. The protocol
notes, USB ids, model table and DPI tables were derived from ATK HUB 3.2.27 and from the USB
traffic, for interoperability. The app writes settings to the mouse's onboard memory and never
writes firmware. Use it at your own risk.

## License

MIT
