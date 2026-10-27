"""Render a 480x320 test frame and dump it as RGB565 little-endian bytes.

Pixel layout must match what the vendor app produces via
Bitmap.LockBits(PixelFormat.Format16bppRgb565): one u16 per pixel, little-endian,
RRRRRGGG GGGBBBBB, rows top-to-bottom, no row padding (480*2 is 4-byte aligned).
"""
import struct, sys
from PIL import Image, ImageDraw, ImageFont

W, H = 480, 320
out = sys.argv[1]

img = Image.new("RGB", (W, H))
px = img.load()
for y in range(H):
    for x in range(W):
        px[x, y] = (x * 255 // (W - 1), y * 255 // (H - 1), 128)

d = ImageDraw.Draw(img)
# colour bars along the top so byte order / channel order errors are obvious
bars = [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0), (0, 255, 255), (255, 0, 255), (255, 255, 255), (0, 0, 0)]
bw = W // len(bars)
for i, c in enumerate(bars):
    d.rectangle([i * bw, 0, (i + 1) * bw - 1, 39], fill=c)
# 1-px white border: checks that x/y/ex/ey packing hits the exact edges
d.rectangle([0, 0, W - 1, H - 1], outline=(255, 255, 255))
try:
    f_big = ImageFont.truetype("arial.ttf", 44)
    f_small = ImageFont.truetype("arial.ttf", 22)
except Exception:
    f_big = f_small = ImageFont.load_default()
d.text((20, 70), "PROTOCOL OK", font=f_big, fill=(255, 255, 255))
d.text((20, 130), "RED should be top-left bar", font=f_small, fill=(255, 255, 255))
d.text((20, 160), "white 1px border on all 4 edges", font=f_small, fill=(255, 255, 255))
d.text((20, 190), "cmd 109 / 121(102,480,320) / 197", font=f_small, fill=(255, 255, 255))
d.text((20, 220), "sent by reverse-engineered client", font=f_small, fill=(255, 255, 255))
d.text((20, 270), "COM20  115200 8N1  RGB565 LE", font=f_small, fill=(0, 0, 0))

buf = bytearray(W * H * 2)
i = 0
for y in range(H):
    for x in range(W):
        r, g, b = px[x, y]
        v = ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3)
        struct.pack_into("<H", buf, i, v)
        i += 2
open(out, "wb").write(buf)
img.save(out + ".png")
print("wrote", len(buf), "bytes to", out)
