#!/usr/bin/env python3
"""Gera o ícone do app Quacko a partir do Pato Pixel (sprite 1x do pacote embutido).

Saída: src-tauri/icons/app-icon.png (1024x1024). Depois rode `npx tauri icon
src-tauri/icons/app-icon.png` para gerar os tamanhos de cada sistema.

Uso (precisa do Pillow; ver seção 8 do quacko-spec.md para o venv):
  python3 tools/make_icon.py
"""
import os
import sys

from PIL import Image, ImageDraw

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SPRITE = os.path.join(ROOT, "src/assets/characters/pato-pixel/assets/info.png")
OUT = os.path.join(ROOT, "src-tauri/icons/app-icon.png")

SIZE = 1024
# Área útil de ícones do macOS: ~824 px centrados, com cantos arredondados.
PLATE = 824
RADIUS = 186
BG_TOP = (214, 236, 255)     # azul-água claro
BG_BOTTOM = (168, 210, 246)  # azul-água um pouco mais forte
FRAME = 28                    # um quadro do sprite (grade 28x28)
SCALE = 24                    # inteiro: o pixel art não deforma (28 x 24 = 672 px)


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
    cat = sheet.crop((0, 0, FRAME, FRAME))  # primeiro quadro do estado "info"
    cat = cat.resize((FRAME * SCALE, FRAME * SCALE), Image.NEAREST)

    icon = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    offset = (SIZE - PLATE) // 2
    icon.alpha_composite(gradient_plate(), (offset, offset))
    # Centraliza pela caixa do desenho (sem as bordas transparentes do sprite).
    left, top, right, bottom = cat.getbbox()
    x = (SIZE - (right - left)) // 2 - left
    y = (SIZE - (bottom - top)) // 2 - top + SCALE  # um pixel abaixo: o gato "senta" na placa
    icon.alpha_composite(cat, (x, y))
    icon.save(OUT)
    print("ok ->", OUT)


if __name__ == "__main__":
    sys.exit(main())
