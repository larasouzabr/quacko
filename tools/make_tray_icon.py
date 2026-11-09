#!/usr/bin/env python3
"""Gera o icone da bandeja (barra de menu) do Pato Pixel.

macOS: "template image" = preto + transparencia; o sistema pinta de branco/preto conforme o tema.
  trayTemplate.png (16), trayTemplate@2x.png (32), trayTemplate@3x.png (48)
  trayAlertTemplate*.png: o mesmo com um pontinho no canto (aviso pendente / falha)
Windows e Linux (nao aceitam template): tray-color*.png, colorido.

Uso: python3 make_tray_icon.py [pasta_de_saida]
"""
import os
import sys

from PIL import Image, ImageDraw, ImageFont

# Perfil do pato virado para a direita: '#' corpo, 'b' bico, 'e' olho (furo)
DUCK = [
    "................",
    "................",
    "..........##....",
    ".........####...",
    ".........#e###..",
    ".........#####..",
    ".........#####bb",
    ".........#####b.",
    ".#......#######.",
    ".##....#######..",
    ".#############..",
    ".#############..",
    "..############..",
    "...###########..",
    "....#########...",
    "................",
]
ALERT_DOT = [(14, 2), (15, 2), (14, 3), (15, 3)]

COLORS = {"#": (255, 214, 74), "b": (255, 140, 42), "e": (43, 32, 36)}
FEET = (255, 140, 42)


def template(alert=False):
    img = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    px = img.load()
    for y, line in enumerate(DUCK):
        for x, ch in enumerate(line):
            if ch in "#b":
                px[x, y] = (0, 0, 0, 255)
    if alert:
        for x, y in ALERT_DOT:
            px[x, y] = (0, 0, 0, 255)
    return img


def colored(alert=False):
    img = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    px = img.load()
    for y, line in enumerate(DUCK):
        for x, ch in enumerate(line):
            if ch in COLORS:
                px[x, y] = COLORS[ch] + (255,)
    if alert:
        for x, y in ALERT_DOT:
            px[x, y] = (255, 90, 70, 255)
    return img


def up(img, s):
    return img.resize((img.width * s, img.height * s), Image.NEAREST)


def tint(img, rgb):
    out = Image.new("RGBA", img.size, (0, 0, 0, 0))
    a = img.split()[3]
    out.paste(Image.new("RGBA", img.size, rgb + (255,)), (0, 0), a)
    return out


def main(out):
    os.makedirs(out, exist_ok=True)
    for name, fn in (("trayTemplate", lambda: template(False)),
                     ("trayAlertTemplate", lambda: template(True)),
                     ("tray-color", lambda: colored(False)),
                     ("tray-color-alert", lambda: colored(True))):
        base = fn()
        for scale, suf in ((1, ""), (2, "@2x"), (3, "@3x")):
            up(base, scale).save(os.path.join(out, f"{name}{suf}.png"))

    # previa: barra escura e clara, em tamanho de tela retina (@2x ampliado)
    try:
        font = ImageFont.truetype("DejaVuSans.ttf", 13)
    except Exception:
        font = ImageFont.load_default()
    S = 6
    cell, bar_h = 16 * S, 16 * S + 24
    gap = 40
    bars = [("barra escura", (20, 32, 54), (255, 255, 255)), ("barra clara", (236, 236, 240), (20, 20, 20))]
    variants = [("normal", template(False), colored(False)),
                ("com aviso", template(True), colored(True))]
    W = 24 + 4 * (cell + gap)
    sheet = Image.new("RGB", (W, 2 * (bar_h + 36) + 12), (255, 255, 255))
    d = ImageDraw.Draw(sheet)
    for r, (bname, bg, fg) in enumerate(bars):
        y0 = 12 + r * (bar_h + 36)
        d.rectangle([0, y0, W, y0 + bar_h], fill=bg)
        x = 24
        for vname, tpl, col in variants:
            for kind, img in (("mono", tint(tpl, fg)), ("cor", col)):
                big = up(img, S)
                tmp = Image.new("RGBA", big.size, bg + (255,))
                tmp.alpha_composite(big)
                sheet.paste(tmp.convert("RGB"), (x, y0 + 12))
                d.text((x, y0 + bar_h + 6), f"{vname} · {kind}", fill=(60, 60, 60), font=font)
                x += cell + gap
        d.text((24, y0 + bar_h + 20), bname, fill=(120, 120, 120), font=font)
    sheet.save(os.path.join(out, "preview-tray.png"))

    # tamanho real (@2x) sobre uma barra, para conferir a leitura
    real = Image.new("RGB", (300, 48), (20, 32, 54))
    x = 16
    for img in (tint(template(False), (255, 255, 255)), tint(template(True), (255, 255, 255)),
                colored(False), colored(True)):
        tmp = up(img, 2)
        real.paste(tmp, (x, 8), tmp)
        x += 64
    real = up(real.convert("RGBA"), 3)
    real.convert("RGB").save(os.path.join(out, "preview-tray-tamanho-real.png"))
    print("ok ->", out)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "tray")
