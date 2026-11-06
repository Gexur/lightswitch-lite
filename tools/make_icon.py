"""Generates res/icon.ico (multi-size) plus a PNG preview.

Draws a split circle: light half / dark half. Readable at 16 px
on both light and dark taskbars.
"""
from __future__ import annotations

import io
import struct
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
RES = ROOT / "res"
SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]
SS = 8  # supersampling

LIGHT = (250, 250, 252, 255)
DARK = (22, 24, 29, 255)
EDGE = (120, 127, 138, 255)


def render(size: int) -> Image.Image:
    canvas = size * SS
    img = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    margin = canvas * 0.045
    box = (margin, margin, canvas - margin, canvas - margin)
    width = max(1, int(canvas * 0.045))

    draw.ellipse(box, fill=LIGHT)
    # Dark right half (pieslice from -90 to 90 degrees).
    draw.pieslice(box, start=-90, end=90, fill=DARK)
    draw.ellipse(box, outline=EDGE, width=width)

    return img.resize((size, size), Image.LANCZOS)


def write_ico(path: Path, images: list[Image.Image]) -> None:
    blobs = []
    for image in images:
        buffer = io.BytesIO()
        image.save(buffer, format="PNG", optimize=True)
        blobs.append(buffer.getvalue())

    count = len(images)
    header = struct.pack("<HHH", 0, 1, count)
    offset = 6 + 16 * count
    entries = b""
    data = b""
    for image, blob in zip(images, blobs):
        width = 0 if image.width >= 256 else image.width
        height = 0 if image.height >= 256 else image.height
        entries += struct.pack("<BBBBHHII", width, height, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
        data += blob

    path.write_bytes(header + entries + data)


def main() -> None:
    images = [render(size) for size in SIZES]
    RES.mkdir(exist_ok=True)
    write_ico(RES / "icon.ico", images)
    render(256).save(RES / "icon-preview.png")
    print(f"icon.ico: {len(SIZES)} tamanos, {(RES / 'icon.ico').stat().st_size} bytes")


if __name__ == "__main__":
    main()
