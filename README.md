# turzx — Turing 3.5" USB display, reverse engineering + Rust rebuild

| Path | What |
|---|---|
| `35inchENG/` | Vendor package as shipped (`UsbMonitor.exe`, themes, driver INF). Untouched. |
| `docs/PROTOCOL.md` | **Wire protocol**: USB identity, serial setup, 6-byte header packing, every command id, RGB565 layout, sequences, live-test results. |
| `docs/APP_INTERNALS.md` | How the vendor app is built: classes, startup, render loop, theme/config file formats, obfuscation, native deps, network. |
| `docs/RUST_REBUILD.md` | Proposed Rust workspace (proto / device / canvas / daemon) and the HTTP API for pushing content. |
| `tools/make_frame.py` | Renders a 480×320 test image and dumps RGB565 LE bytes (needs Pillow). |
| `tools/send_frame.ps1` | Minimal working client: screen-on, orientation, full frame over COMx (pure .NET, no install). |
| `tools/decode_strings.py` | Dotfuscator string decryptor used on the ILSpy output. |
| `reference/decompiled-src/` | ILSpy C# of `UsbMonitor.exe` with all strings decrypted (identifiers still obfuscated). |

Quick test (device on COM20):

```bash
python tools/make_frame.py frame.bin && powershell -File tools/send_frame.ps1 -Port COM20 -Frame frame.bin
```
