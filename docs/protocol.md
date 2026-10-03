# COMPX HID protocol

Notes for ATK / VXE / MAD / Dragonfly mice, as observed from ATK HUB 3.2.27 and checked on an
ATK Y9 Ultimate with the 8K dongle (VID `0x373B`, PID `0x11D9`).

## Transport

- Output report `0x08`, 16 bytes after the report ID; replies come as input report `0x08` and echo
  the command byte.
- On Windows each top-level collection is its own handle. ATK HUB filters on usage page `0xFF04`,
  but the collection that accepts report `0x08` is `0xFF02 / 0x02`; the others fail the write with
  `ERROR_INVALID_USER_BUFFER` (1784). `Mouse::open` tries each vendor collection.

```
[0x08] [cmd] [status] [addr_hi] [addr_lo] [len] [payload × 10] [checksum]
checksum = 0x55 - (0x08 + sum of the 15 bytes after the report ID) mod 256
```

In the 17-byte buffer, payload byte *k* is at index `6 + k`. A sleeping mouse drops the first
packets, so reads are retried.

## Commands

| cmd  | name | request | reply payload |
|------|------|---------|---------------|
| 0x01 | handshake | `len 8`, 4 arbitrary bytes | `[4]` cid, `[5]` mid, `[6]` connection type |
| 0x03 | online | — | `[0]` 1 = mouse linked |
| 0x04 | battery | — | `[0]` %, `[1]` charging, `[2..3]` mV (BE) |
| 0x07 | set EEPROM | addr, len, data | — |
| 0x08 | get EEPROM | addr, len | data |
| 0x0A | status report (unsolicited) | — | `[0]` flags: `0x01` DPI, `0x02` polling, `0x04` profile, `0x40` battery |
| 0x0E | get profile | — | `[0]` 0-based profile |
| 0x0F | set profile | `[index]` | often no reply; confirm with 0x0E |
| 0x10 | get CID/MID | — | `[0]` cid, `[1]` mid, `[6..9]` unique id |
| 0x12 | mouse firmware | — | `[0].[1]` → `format!("{:x}.{:02x}")` |
| 0x1D | receiver firmware | — | same format |

Connection types: 0 dongle 1K, 1 dongle 4K, 2 wired 1K, 3 wired 8K, 4 dongle 2K, 5 dongle 8K,
6 wired 2K, 7 wired 4K, 8 NearLink. Polling rates offered are those up to the link's maximum.

COMPX mice have 4 onboard profiles. EEPROM addresses always refer to the **active** profile.

## EEPROM (active profile)

Values are stored as `[v, 0x55 - v]` pairs unless noted. Writes are read-modify-write of the whole
block, then read back and compared.

| addr | len | content |
|------|-----|---------|
| 0x00 | 10 | polling, stage count, current stage (0-based), bhop (ms/10), button mode |
| 0x0A | 2 | lift-off (raw, see below) |
| 0x0C / 0x14 / 0x1C / 0x24 | 8 | DPI stages 1-2 / 3-4 / 5-6 / 7-8, 4 bytes each |
| 0xA9 | 10 | debounce ms, motion sync, LED-off time (s/10), angle snapping, ripple |
| 0xB5 | 6 | sensor sleep on, sleep time (s/10), sensor mode (0 basic, 1 competitive, 2 competitive max) |
| 0x1B00 + 6·i | 6 | PAW3955 DPI stage *i* |

Polling byte: `0x08` 125, `0x04` 250, `0x02` 500, `0x01` 1000, `0x10` 2000, `0x20` 4000, `0x40` 8000.

Debounce options in ATK HUB: 0, 1, 2, 4, 8, 15, 20 ms.

### DPI stage (classic sensors)

`[x, y, ex, 0x55 - (x + y + ex)]`. `ex` holds one nibble per axis (X low, Y high): bit 0 double,
bit 1 high range (PAW3950 Ultra) or double (table sensors), bits 2-3 register bits 8-9.

| sensor | decode `v = x | hi << 8` | steps |
|--------|--------------------------|-------|
| PAW3950 Ultra | `bit1 ? v*50 + 10050 : (v+1)*10`, `bit0` ×2 | 10 → 10000, 50 → 30000, 100 → 42000 |
| PAW3950 / 3950DM / 3395 / 3395 Ultra | `(v+1)*50`, `bit0` ×2 | 50 (100 above 30000) |
| PAW3395SE / 3311 / 3315 | lookup table, `bit1` ×2 | 50 → 10000, doubled 5050–9000 |

### DPI stage (PAW3955)

`[x_lo, x_hi, y_lo, y_hi, flags, crc]`, `dpi = (v + 1) × (flag ? 2 : 1)` with flag `0x01` (X) /
`0x10` (Y); values ≥ 40000 use the doubled form.

### Lift-off

- PAW3950 Ultra / PAW3955: raw 1–11 → 0.7–1.7 mm.
- PAW3950 / 3950DM: 3 → 0.7 mm, 1 → 1 mm, 2 → 2 mm.
- PAW3395 family: 1 → 1 mm, 2 → 2 mm.
- PAW3311 / 3315: none.

## Firmware update service (ATK HUB)

Unauthenticated `GET https://api-hub-cloud.atk.pro/firmware/{vid}/{pid}/{family}` (decimal ids),
answering JSON whose `data.version` is the latest version (e.g. `"3.03"`); only that field is used
here. The mouse is looked up by its own USB id and firmware family (`y9` for the Y9 Ultimate,
`14139/4559/y9`), the receiver by its USB id and `dongle`. Versions compare as dot-separated
numbers; only a strictly greater one is an update.
