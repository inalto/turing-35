# How UsbMonitor.exe works (internals)

Target: `35inchENG/UsbMonitor.exe`, SHA-256 `e2b6ebf6425f397e57fd1eb7beab11888b6c5b6fe0b3bdb3032cefba7ba4b2c0`,
1 609 848 bytes, PE32+ x64, IL-only, .NET Framework 4.8, module name `usb_hid.exe`, assembly `usb_hid 1.0.0.0`.
REA evidence: artifact `ev_cc8d9e35…6676`, native boundaries `ev_c1d74223…7b400`, members `ev_00507936…1125`.

Decompiled with ILSpy 11.2 (`ilspycmd -p`), then every `r.b("…", key)` call was replaced by its plaintext
with `tools/decode_strings.py`; the result is `reference/decompiled-src/` (identifiers are still the
obfuscated names, strings are readable, marked `/*dec*/`).

## 1. Obfuscation

* **Dotfuscator** (attribute `DotfuscatorAttribute`): identifiers renamed to Hangul/Latin-ext glyphs, control
  flow flattened into `switch(num)` state machines, dead `short num = …; switch(num == num)` junk.
* String encryption: `r.b(string s, int key)` in `r.cs:1033`. Per character `c`, with running counter
  `n = 2055530166 + key + 87`: `lo = (c & 0xFF) ^ n++`, `hi = (c >> 8) ^ n++`, output char = `(lo << 8) | hi`.
  Only the low 8 bits of `n` matter. Each method has its own `int a_ = key;` local.
* No packing, no anti-debug, no integrity check. Nothing is verified online.

## 2. Process layout

| Piece | Obfuscated name | File | Role |
|---|---|---|---|
| Entry point | `괗._A839()` | `괗.cs` | `[STAThread]` Main: UTF-8 console, hides console window if it owns one, `Application.Run(new c())` |
| Main window | `c : Form` | `c.cs` (17 k lines) | Theme picker, settings, preview, debug buttons; owns the Monitor |
| Monitor / renderer | `_꧶ : IDisposable` | `꧶.cs` (6.6 k lines) | Creates the device, samples sensors, composes the frame, drives the update loop. Singleton `_꧶.랅`; app config singleton `_꧶._AA4F`; theme dir `_꧶.ꥄ` |
| Serial base class | `_A8EB` | `-.cs:1077-1832` | Holds `SerialPort`, WMI enumeration, locked `Write` |
| Device class | `ꪨ : _A8EB` | `ꪨ.cs` | Header packing, all command helpers, frame buffer (`byte[] _AA4F`), composed bitmap, glyph cache `Dictionary<string, byte[]> ꬁ`; singleton `ꪨ.ꭚ` |
| Bitmap→bytes | `_AA4F` | `-.cs:1834-2117` | `LockBits` wrapper; `ꥄ(2)` locks as `Format16bppRgb565` and copies to a byte array |
| Image/text helpers | `ꢒ` | `ꢒ.cs` | RGB565 conversion, text rendering into the frame bitmap, sub-rectangle slicing, bitmap diff |
| Theme model | `usb_hid.ThemeConfig`, `TGroup`, `TItem`, `FontConfig`, `StatuBar`, `ArchBar`, `ScreenTip` | `usb_hid/*.cs` | Serializable layout objects (see §5) |
| App config | `usb_hid.AppConfig` | `usb_hid/AppConfig.cs` | Persisted settings (see §5) |
| Sensors | `M_Data`, `y`, `군`, `귉`, `뀸`… | various | WMI/perf counters, NVML (`nvml.dll` P/Invoke), `disk.dll` (`InitDiskTemp/GetDiskTemp`, **not shipped** in this folder), optional AIDA64 shared memory (`usedAida*` flags), NAudio for volume |
| Persistence | `ꥄ` | `ꥄ.cs` | `BinaryFormatter` to/from `byte[]` |
| Logger | `ꬁ` | `ꬁ.cs` | `usb_hid_logs/logs/yyyyMMdd_log.txt`, levels via `LogLevel` |
| Localization | `귉` | `귉.cs` | XML string table keyed by `긢._A839` (region code) |
| Secondary forms | `f`, `u`, `j`, `n`, `r`, `SettingForm` | | Theme editor, "native widget" test forms, weather city picker, settings |

Native boundaries (REA `inspect_managed_native_boundaries`): `nvml.dll` (10 NVML functions),
`disk.dll` (2), `kernel32` (`GetConsoleWindow`, `QueryPerformanceCounter/Frequency`), `user32`
(`GetWindowThreadProcessId`), `Crypt32` (`CryptProtectData/UnprotectData` — used by a JSON
`EncryptedData` helper in `녃.cs`, DPAPI for a stored secret such as the weather API key [INF]), and 15
ordinal imports from `msvcr120d.dll` (debug CRT — explains why the debug CRT DLLs ship with the app).

## 3. Startup sequence

1. `c` constructor loads `config/AppConfig.data` (`꧶.cs:6708`), scans `config/*.data` for themes
   (`c.cs:10965`), builds the preview list, applies `autoRun` (Task Scheduler via
   `Microsoft.Win32.TaskScheduler.dll`, task name `AutoRuner`, argument `-auto`).
2. `new _꧶(ThemeConfig)` (`꧶.cs:2955`): picks the device by `AppConfig.deviceIndex`
   (1 → `new ꪨ("USB35INCH")`, 0 → `new ꪨ("USB24INCH")`), selects the NIC named in
   `netcardName` for traffic counters.
3. Connect state machine (`c.cs:355-460`): `뗈().귉()` (close + enumerate + open), then
   `뗈().ꭚ()` (send 255, 10 s timeout), retried once.
4. Monitor loop task (`꧶.cs:230-560`), see PROTOCOL.md §5.2 for the wire sequence:
   brightness 0 → 109 → mirror/orientation → compose → brightness `blkValue` → full 197 → "show full done".

## 4. Render loop

* Runs on a `Task`; period `1000 - elapsed` ms (`꧶.cs:296`), minimum clamp, plus a 500 ms branch.
* Sensor sampling (`샨()` and friends) fills the `_꧶` value fields; each `TItem` formats its value
  (`valueStr`), compares with `lastValueStr`, and only re-renders on change.
* Text rendering: `ꢒ.ꞇ(Bitmap frame, string text, int size, Color, x, y, offX, offY, W, H, …)`
  draws the text onto a copy of the composed background, then `ꢒ._A839(bitmap)` converts that
  `W×H` box to RGB565 bytes. Result cached in `ꪨ.ꭚ.ꬁ[text + x + y (+ font + colour)]`, so a
  repeated value costs no GDI work. Sent with 197 at `(x, y, W, H)` (`TItem.cs:1012-1129`).
* `StatuBar` (horizontal bar): keeps `frontData`/`backData` = per-row colours and `backFullData`
  (background crop). Only the delta segment between `displayLen` and `curLen` is sent (`StatuBar.cs:1067-1180`).
* `ArchBar` (arc gauge): on theme load it draws 100 arcs with GDI+ into a 300×300 scratch bitmap
  and records the black pixels as `(dx, dy)` byte pairs (`ArchBar.cs:1590-1760`); at runtime it
  streams the pairs between the old and the new percentage with command 195 in front or back colour,
  28 points per 64-byte packet.
* Every ~12 h a full redraw is forced; a brightness change from the UI sends 110 immediately.
* `쉌()` stops the loop (used before the "native widget" test commands and on exit).

## 5. Data files

### `config/AppConfig.data` — `BinaryFormatter` of `usb_hid.AppConfig`

`CityName, CityWeatherCode, themeIndex, themeName, blkValue (0-255), noUpdate, invertLcd, rotation,
deviceIndex (1 = 3.5"), mirrorMode, netcardName, netcardIndex, autoRun, isUnReadHardDisk,
barSaveOffset, customColor[], CpuTempSensorSubName, CpuFanSensorSubName, CpuPumpSensorSubName,
usedAidaCPU, usedAidaGPU, usedAidaHDD, GpuMemName, GpuName, PartionName, HDDIndex, FahTemp`.

### `config/<theme>.data` — `BinaryFormatter` of `usb_hid.ThemeConfig`

```
ThemeConfig { isLanscape, deviceIndex, tipList: List<ScreenTip>, groupList: List<TGroup>,
              barList: List<StatuBar>, archList: List<ArchBar>, osdImg, backImg, osdWithFontBack: Bitmap,
              enbaleOsdWithFontBack, useOsdWithFontBack, needRereshArch, name, userInfoCnt,
              barType, barFrontIndex, barBackIndex, barFrontColor, barBackColor }
TGroup      { enabled, groupName, backImg, baseX, baseY, itemList: List<TItem> }
TItem       { enabled, isLongStr, itemName, strLen, cnName, posX, posY, flashDataOffset,
              fontType: FontConfig, intervalAll, valueStr, lastValueStr }
FontConfig  { name, size, color, offsetX, offsetY, interval, W, H }
StatuBar : TItem { displayLen, curLen, stepWidth, length, height, frontData, backData, backFullData,
              backImgData, backColor, frontColor, lastTime }
ArchBar  : TItem { startPer, useInternal, offsetX, offsetY, archR, archWidth, barType, curPer,
              displayPer, backArchColor, frontArchColor, dataLen, dataLenArray, archData, curFlashPos }
```

`itemName` selects the sensor (CPU temp, GPU load, time, weather, …); the mapping lives in the
item-update switch in `꧶.cs` (not transcribed). Background PNGs in `3.5/` are the theme backdrops.

`report.data` (next to the exe) is a `BinaryFormatter` `Dictionary<string,string>` cache, not a license.

### `code.ini`
Single integer (`64`); the troubleshooting note says to set it to `0` or `8192` if the app hangs at
init. It is read by the sensor layer (likely a WMI/perf-counter mode flag) [INF].

## 6. Network

Only three endpoints, all optional (weather + update check):
`http://www.turzx.com/update_35.html`, `https://api.seniverse.com/v3/...` (location search, now),
`https://n93yfu2pad.re.qweatherapi.com/...` (geo lookup, weather now). The API key is stored DPAPI-encrypted.

## 7. Threading and locking

* All serial writes go through `_A8EB.ꪨ(byte[], bool)` under a static lock (`-.cs:1721-1789`).
* Command builders (`ꪨ.ꪨ(int cmd, …)`) take an instance lock `m_갌` as well, so a header + data pair
  from the pixel path (`ꪨ.cs:1803`) is **not** atomic with respect to a control command from the UI
  thread — the two sleeps around the data write exist partly to tolerate that [INF].
* The glyph cache is never invalidated except on theme change.

## 8. What a rebuild does not need

* Dotfuscator, DPAPI, BinaryFormatter, BouncyCastle (only used by the encrypted-JSON helper),
  TaskScheduler, NAudio, the debug CRT, `disk.dll`.
* The "native widget" commands (130/131/161–212).
* WMI: on Windows, USB VID/serial are available from the serial-port enumeration API directly.
