#!/usr/bin/env python3
import os
import sys

from PIL import Image, ImageDraw

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SPRITE = os.path.join(ROOT, "src/assets/characters/pato-pixel/assets/info.png")
OUT = os.path.join(ROOT, "src-tauri/icons/app-icon.png")

SIZE = 1024
PLATE = 824
RADIUS = 186
BG_TOP = (214, 236, 255)
BG_BOTTOM = (168, 210, 246)
FRAME = 28
SCALE = 24


def gradient_plate():
    plate = Image.new("RGBA", (PLATE, PLATE))
    draw = ImageDraw.Draw(plate)
    for y in range(PLATE):
        t = y / (PLATE - 1)
        color = tuple(round(a + (b - a) * t) for a, b in zip(BG_TOP, BG_BOTTOM))
        draw.line([(0, y), (PLATE, y)], fill=color + (255,))
    mask = Image.new("L", (PLATE, PLATE), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, PLATE - 1, PLATE - 1], RADIUS, fill=255)
    plate.putalpha(mask)
    return plate


def main():
    sheet = Image.open(SPRITE).convert("RGBA")
    cat = sheet.crop((0, 0, FRAME, FRAME))
    cat = cat.resize((FRAME * SCALE, FRAME * SCALE), Image.NEAREST)

    icon = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    offset = (SIZE - PLATE) // 2
    icon.alpha_composite(gradient_plate(), (offset, offset))
    left, top, right, bottom = cat.getbbox()
    x = (SIZE - (right - left)) // 2 - left
    y = (SIZE - (bottom - top)) // 2 - top + SCALE
    icon.alpha_composite(cat, (x, y))
    icon.save(OUT)
    print("ok ->", OUT)


if __name__ == "__main__":
    sys.exit(main())
