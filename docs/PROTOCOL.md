# Turing 3.5" USB display (USB35INCHIPSV2) — wire protocol

Reverse-engineered from `35inchENG/UsbMonitor.exe` (assembly `usb_hid`, .NET Framework 4.8,
Dotfuscator-obfuscated) and **verified live on the real panel on 2026-10-27** (COM20).
Evidence tags: `[OBS]` observed in decompiled code or on hardware, `[INF]` inferred, `[UNK]` unknown.

Decompiled, string-decrypted source is in `reference/decompiled-src/`. File references below use
those files. The device driver class is `ꪨ.cs` (derives from `_A8EB` in `-.cs`).

---

## 1. Transport

| Item | Value | Evidence |
|---|---|---|
| Bus | USB, CDC-ACM class (`USB\Class_02&SubClass_02`), stock Windows `usbser.sys` | `Driver/usbser/cdc.inf` [OBS] |
| USB identity | `VID_1A86` (WCH), **PID_5722** on the unit tested; the app also special-cases `PID_F111` | `-.cs` port enumeration + live `Win32_SerialPort` [OBS] |
| PnP instance / serial string | `USB35INCHIPSV2` (becomes the device serial in `PNPDeviceID`) | `-.cs:1232..1530` [OBS] |
| Windows friendly name | `USB Serial Device (COMnn)` | live [OBS] |
| Serial params the app sets | 115200 baud, 8 data bits, no parity, 1 stop bit, **DTR=1, RTS=1**, ReadTimeout 1000 ms | `ꪨ.cs:1175-1185` [OBS] |
| Effective throughput | 307 200-byte frame written in 2.4 s ≈ 128 KB/s (USB CDC ignores the baud figure) | live [OBS] |
| Direction | **Host → device only.** The app never reads the port; the panel sent 0 bytes back after a full frame | no `Read`/`DataReceived` anywhere; live `BytesToRead == 0` [OBS] |
| Flow control | none; every command is a single `SerialPort.Write` | `-.cs:1736` [OBS] |

### 1.1 Device discovery (as implemented by the vendor app)

`_A8EB.꿟()` in `-.cs`:

1. WMI query `select * from Win32_SerialPort`; for each row read `DeviceID` (e.g. `COM20`) and `PNPDeviceID`.
2. Accept the row if `PNPDeviceID` contains the constructor string — `"USB35INCH"` for the 3.5" model, `"USB24INCH"` for the 2.4" model (`꧶.cs:2982-2987`, chosen by `AppConfig.deviceIndex`: 1 → 3.5", 0 → 2.4").
3. Fallback: if `PNPDeviceID` contains `"VID_1A86&PID_F111"` the row is also accepted, and the app
   rewrites the name to `<prefix up to PID_F111>\USB35INCHIPSV2` so later checks pass (older units without the IPSV2 serial string [INF]).
4. First match wins; its `COMnn` is opened.
5. The matched `PNPDeviceID` is kept in `_A839`. If it contains `"V2"` the app skips the inter-packet
   sleeps (see §4); otherwise it sleeps 10 ms after the header and after the pixel data.
6. A localized `StrRomLimit` exception is thrown when a global flag `긢._A839 == 1` and the name contains `"CHS"` (vendor region/ROM lock — irrelevant for a rebuild) [OBS, purpose INF].

A rebuild can simply filter serial ports by USB VID `0x1A86` and serial string containing `USB35INCH`.

---

## 2. Packet format

Every command starts with a **6-byte header**. Four geometry fields are bit-packed, the sixth byte
is the command id. Reconstructed from `ꪨ.ꪨ(int cmd, int x, int y, int ex, int ey, byte[] buf, int)` at `ꪨ.cs:1683-1801`:

```
byte 0 :  x  >> 2
byte 1 : ((x  & 0x03) << 6) | (y  >> 4)
byte 2 : ((y  & 0x0F) << 4) | (ex >> 6)
byte 3 : ((ex & 0x3F) << 2) | (ey >> 8)
byte 4 :   ey & 0xFF
byte 5 :   command id
```

i.e. `x` is 10 bits, `y` is 10 bits, `ex` is 10 bits, `ey` is 9 bits + the low byte (effectively ≤ 0x1FF… the app only ever uses values ≤ 479).
Worked examples (verified against the formula):

| cmd, x, y, ex, ey | bytes |
|---|---|
| 197, 0, 0, 479, 319 (full landscape frame) | `00 00 07 7D 3F C5` |
| 197, 20, 20, 119, 69 (100×50 rect at 20,20) | `05 01 41 DC 45 C5` |
| 121, 0, 0, 0, 0 (orientation, header part) | `00 00 00 00 00 79` |

If a caller supplies a buffer, the header is written **into bytes 0..5 of that buffer** and the whole
buffer is sent as one write; otherwise a fresh 6-byte buffer is sent. Writes are serialized with a
process-wide lock (`_A8EB.ꞇ`) so UI threads and the render loop never interleave packets.

**Byte order inside payloads**: all multi-byte numbers in fixed-size payloads are big-endian
(width/height in cmd 121, colours in cmd 195). Pixel data (cmd 194/197/198) is **little-endian RGB565** because it is a raw copy of a GDI+ `Format16bppRgb565` bitmap (see §3).

---

## 3. Pixel format

* 16 bits per pixel, `RRRRRGGG GGGBBBBB` (`ꢒ.ꞇ(Color)` at `ꢒ.cs:157`: `((R<<8)&0xF800)|((G<<3)&0x7E0)|(B>>3)`).
* The frame buffer bytes are produced by `Bitmap.LockBits(..., PixelFormat.Format16bppRgb565)` and
  `Marshal.Copy` (`-.cs:2089-2097`, class `_AA4F`), so on the wire each pixel is a **little-endian u16**:
  low byte first. Rows are top-to-bottom, left-to-right, no padding (480·2 and 320·2 are 4-byte aligned).
* A sub-rectangle update sends only the rectangle's pixels, row-major, `w*h*2` bytes (`ꢒ.ꞇ(byte[] fb, int fbWidth, x, y, w, h, 2)` at `ꢒ.cs:3201` slices rows out of the full frame buffer).
* Logical screen size is set by the host: landscape `480×320`, portrait `320×480` (`ꪨ.ꪨ(bool isLandscape)` at `ꪨ.cs:1326`). The 2.4" model uses the same code and the same constants [OBS]; its true panel size was not verified [UNK].

Live test: the frame generated by `tools/make_frame.py` (preview `tools/test_frame_preview.png`) was
accepted; channel order and edge alignment can be checked on the panel from that image.

---

## 4. Commands

### 4.1 Control commands (6-byte header only, unless noted)

| id | Name (vendor semantics, [INF] unless noted) | x | y | ex | ey | Payload | Where used |
|---|---|---|---|---|---|---|---|
| 101 | RESET | 0 | 0 | 0 | 0 | – | `꧶.cs:4587` (`솚()`) |
| 102 | CLEAR | 0 | 0 | 0 | 0 | – | `ꪨ.cs:1595` default arg |
| 103 | TO_BLACK | 0 | 0 | 0 | 0 | – | shutdown sequence `꧶.cs:3089` |
| 108 | SCREEN_OFF | 0 | 0 | 0 | 0 | – | shutdown `꧶.cs:3088` |
| 109 | SCREEN_ON | 0 | 0 | 0 | 0 | – | wake `꧶.cs:3060`; verified live |
| 110 | SET_BRIGHTNESS | **level** | 0 | 0 | 0 | – | `꧶.cs:3676`. `x` = 0‥255; the app sends **0 on wake and 255 before SCREEN_OFF**, UI slider max 255 → 0 = brightest, 255 = darkest [INF] |
| 121 | SET_ORIENTATION | 0 | 0 | 0 | 0 | 16 bytes: `[6]=100+orient`, `[7..8]=width BE`, `[9..10]=height BE`, rest 0 | `ꪨ.cs:1438-1477`; verified live with `102,480,320` |
| 122 | SET_MIRROR | 0 | 0 | 0 | 0 | 16 bytes: `[6]=0/1` | `ꪨ.cs:1398-1436`; sent with 1 when `AppConfig.mirrorMode`, else 0 (`꧶.cs:3587/3614`) |
| 130 | (vendor "mode A") | value | value2 | 0 | 0 | – | `ꪨ.cs:1659` (`깻()`, no args) and `c.cs:5892/6225` with a sensor value in `x` — semantics unknown [UNK] |
| 131 | (vendor "mode B") | 0 | 0 | 0 | 0 | – | `ꪨ.cs:1680` (`껔()`), paired with 130 as on/off from a settings button (`c.cs:8820/8842`) [UNK] |
| 255 | PROBE / NO-OP | 0 | 0 | 0 | 0 | – | "test connect": the app sends this after 100 ms and treats a successful write as *connected* (`ꪨ.cs:515`, 10 s timeout). The device never replies |

Orientation values (`꧶.cs:3540-3625`, `SerRotation`): start at 0; landscape → 2; `AppConfig.invertLcd` → +1.
So 0 = portrait, 1 = portrait flipped, 2 = landscape, 3 = landscape flipped; the byte on the wire is **100 + value**.
Width/height sent with 121 are the *logical* size for that orientation (480×320 for 2/3, 320×480 for 0/1).

### 4.2 Pixel commands

| id | Name | Geometry in header | Data | Notes |
|---|---|---|---|---|
| **197** | DISPLAY_BITMAP | `x, y, x+w-1, y+h-1` | header write, then `w*h*2` bytes RGB565 LE in a **second write** | Main command. `ꪨ.cs:1803-1893`. Sleep `A_5` ms (default 10) after header and after data **only if** the port name lacks `V2`. Used for full frames (`긢()` → `0,0,480,320`) and every partial update |
| 198 | DISPLAY_BITMAP_NOWAIT | same | same, no sleeps | `ꪨ.cs:1895-1915`; used for the large OSD digits (`c.cs:3462`, 68×90 and 48×66 glyphs) |
| 194 | DISPLAY_BITMAP_INLINE | same | pixel buffer sent in **one** write with the header overwriting its first 6 bytes | `ꪨ.cs:1917-1936`. Resolves for 5-argument calls only; no live caller found in this build [OBS, dead code INF] |
| **195** | DRAW_PIXELS (arc gauge) | `x=originX, y=originY, ex=byteCount (≤56), ey=0` | fixed **64-byte** packet: `[6]=colour hi, [7]=colour lo` (RGB565 **big-endian**), `[8..8+ex)` = pairs `(dx, dy)` relative to origin | `ArchBar.cs:1081-1144`. The host pre-renders 100 arc segments (one per percent) into offset lists; it sends up to 28 points per packet in either the front or the back colour. Firmware plots those pixels |

### 4.3 Vendor "native widget" commands (observed, semantics not confirmed) [UNK]

These are sent from secondary forms (`c.cs` 3100–8700, `u.cs`) after the render loop is stopped (`쉌()`).
They carry **decimal digits** of the clock and sensor values in fixed-size payloads, which strongly suggests a firmware mode that draws numbers itself. Payload layouts as built by the app:

| id | Payload length | Layout (byte index → content) |
|---|---|---|
| 161 | header only | `x = index+1`, `y = count`, `ex = tempA / 10`, `ey = tempA % 10` (`c.cs:6822`) |
| 162 | ≥ 13 | `[12] = value` (`c.cs:4007`) |
| 164 | 20 | `[6..9] = HH MM as single digits`, `[10..13] = two sensor values, tens/units swapped` (`c.cs:4754-4763`) |
| 166 | 20 | same as 164 plus header `x,y,ex = (byte)sensor values` (`c.cs:4381-4390`) |
| 169 | 20 | `[6..9] = two sensor values`, `[10..13] = HH MM` (`c.cs:5121-5130`) |
| 180 | 21 | `[6..20]` = digits of five sensors (temp %100/10, %10, *10%10; others /100, %100/10, %10) (`c.cs:3158-3174`) |
| 201 | 10 or header only | `[6..8] = 0` (`u.cs:494`, `c.cs:7826`) |
| 212 | header only | `꧶.cs:3114` (`뿝()`), `c.cs:8689` |

Sensor getters in `꧶.cs`: `럞()`→field `깻`, `렷()`→`껔`, `릛()`→`뀸`, `맴()`→`낑`, `밊()`→`m_늧` (CPU/GPU temperature, load, etc. — mapping not resolved) [UNK].
None of these are needed to drive the panel as a plain framebuffer.

---

## 5. Sequences the vendor app uses

### 5.1 Connect (form startup, `c.cs:355-460`)
1. `귉()` — close any open port, re-enumerate (§1.1), open the match.
2. `ꭚ(10)` — "test connect": wait 100 ms, send **255**; success = no write exception within 10 s.
3. Retries once; on failure the UI shows a reconnect prompt.

### 5.2 Wake / first frame (`꧶.cs` monitor loop, lines 230-560)
1. **110** brightness `0`, then **109** SCREEN_ON (`뼫()`).
2. `SerRotation`: **122** mirror (0/1), then **121** orientation+size.
3. Compose the full theme into the 480×320 bitmap, convert to RGB565.
4. **110** with `AppConfig.blkValue`, then **197** full frame (`긢()`), log "show full done".

### 5.3 Steady state
* Loop period ≈ 1000 ms (`delay = 1000 - elapsed`, `꧶.cs:296`), with a 500 ms wait in one branch.
* Each text item re-renders only its own glyph box (`FontConfig.W × H`) from a cache keyed by text+position, and sends it with **197** (`TItem.cs:1012-1129`).
* Bars send only the changed segment as a solid colour block via **197** (`StatuBar.cs:1095-1175`); arc gauges send point lists via **195**.
* A full redraw is forced when the last full draw is older than 12 h (`꧶.cs:278-288`) [INF].

### 5.4 Sleep / exit (`뾄()`, `꧶.cs:3085-3089`)
1. **110** brightness `255`.
2. **121** orientation with `isLandscape=false` geometry (portrait 320×480) — note `ꪨ(0, w, h)` sends orientation byte **100**.
3. **108** SCREEN_OFF, **103** TO_BLACK.

---

## 6. Minimal client (what you need to drive the panel)

```
open COMx 115200 8N1, DTR+RTS high
send hdr(109,0,0,0,0)                                  # screen on
send hdr(121,0,0,0,0) + [102, 0x01,0xE0, 0x01,0x40] + 5×0   # landscape 480×320  (16 bytes total)
send hdr(110, level,0,0,0)                             # optional brightness (0 = max)
send hdr(197, x, y, x+w-1, y+h-1)                      # then
send w*h*2 bytes RGB565 little-endian
```

`tools/send_frame.ps1` + `tools/make_frame.py` implement exactly this and were used for the live test.

---

## 7. Open questions

* Real meaning of 130/131/161–212 (firmware widgets) — not needed for a framebuffer driver.
* Whether the panel tolerates a 197 header and data in one write (194 path) on the V2 firmware — untested.
* Maximum safe rectangle count per second; the vendor app paces itself to ~1 Hz with small rects plus 10 ms gaps on non-V2 units.
* The 2.4" model's native resolution.
