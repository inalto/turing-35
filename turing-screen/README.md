# turing-screen

Rust driver for the Turing 3.5" USB display (`USB35INCHIPSV2`, WCH VID `1A86`).
Protocol reference: [`../docs/PROTOCOL.md`](../docs/PROTOCOL.md). Roadmap: [`../docs/RUST_REBUILD.md`](../docs/RUST_REBUILD.md).

| Crate | What |
|---|---|
| `turing-proto` | `no_std` wire protocol: 6-byte header packing, every command, RGB565 encoding. Byte-exact tests against hardware-verified vectors. |
| `turing-device` | Discovery (`discover()`), serial transport (`Display::open`), `init`, `blit`, `show_rgb`. Generic over `std::io::Write`, tested on an in-memory sink. |
| `turing-canvas` | RGB canvas: `fill`, `fill_rect`, `draw_rgb`, anti-aliased `draw_text` (fontdue, bundled Roboto), `measure_text`. `flush()` diffs against the last RGB565 frame and sends only dirty row bands (full frame above 40 % change). |

| `turing-daemon` | HTTP API binary. One render thread owns the `Display` and a `Canvas`; axum handlers send it operations and await the reply. Tested end-to-end against an in-memory serial sink. |

```bash
cargo test --workspace
cargo run -p turing-device --example show_png -- ../tools/test_frame_preview.png        # auto-discover
cargo run -p turing-device --example show_png -- picture.jpg COM20                      # explicit port
cargo run -p turing-canvas --example counter -- 10                                       # dirty-rect demo
cargo run -p turing-daemon -- --listen 127.0.0.1:8686 [--port COM20] [--orientation portrait]
```

## HTTP API (`turing-daemon`)

| Method | Path | Body / query | Reply |
|---|---|---|---|
| GET | `/v1/device` | – | `{port, serial, connected, orientation, brightness, power, size:{w,h}}` |
| POST | `/v1/device/power` | `{"on": true}` | `{ok}` |
| POST | `/v1/device/brightness` | `{"level": 0..255}` (0 = brightest, vendor convention) | `{ok}` |
| POST | `/v1/device/orientation` | `{"orientation": "landscape" \| "portrait" \| "landscape_flipped" \| "portrait_flipped"}` (clears the canvas) | `{ok, sent}` |
| POST | `/v1/display/clear` | `{"color": "#RRGGBB"}` (default black) | `{ok, sent:[rects]}` |
| POST | `/v1/display/text` | `{"x","y","text","size"(px, default 24),"color","bg","bg_rect":{x,y,w,h}}` | `{ok, rect, sent}` |
| POST | `/v1/display/image` | raw PNG/JPEG/BMP body; query `fit=contain\|cover\|stretch\|none`, `x`, `y`, `bg=%23RRGGBB` | `{ok, sent}` |

Errors are `400 {"error": "..."}` for bad input and `502` when the display write fails (`connected` then reads `false`).

```bash
curl -X POST -H 'content-type: application/json' -d '{"x":20,"y":30,"text":"hello","size":36,"color":"#FFD23C"}' http://127.0.0.1:8686/v1/display/text
curl -X POST -H 'content-type: image/png' --data-binary @photo.png 'http://127.0.0.1:8686/v1/display/image?fit=cover'
```
