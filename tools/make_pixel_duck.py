#!/usr/bin/env python3
"""Gera o personagem original "Pato Pixel" (pixel art, de frente, 4 estados, 2 quadros cada).

Mesmo molde do gato: grade 28x28, contorno escuro automatico, .quackopack schema 1.
Desenho simetrico (espelho em x -> 27 - x).

Saidas (em ./quacko-characters):
  pato-pixel/manifest.json, preview.png, assets/{ok,fail,attention,info}.png
  preview-pato.png, preview-pato.gif, pato-pixel.quackopack (zip)

Uso: python3 make_pixel_duck.py [pasta_de_saida]
"""
import json
import os
import sys
import zipfile

from PIL import Image, ImageDraw, ImageFont

W = H = 28
OY = 1
SCALE = 6
STATES = ["ok", "fail", "attention", "info"]

PAL = {
    "Y": (255, 214, 74),    # penas
    "y": (240, 177, 44),    # sombra das penas (asas, queixo)
    "L": (255, 238, 160),   # barriga e brilho
    "O": (255, 140, 42),    # bico e patas
    "o": (214, 98, 26),     # bico de baixo e sola
    "P": (247, 168, 176),   # bochechas
    "K": (43, 32, 36),      # contorno e olhos
    "B": (110, 198, 240),   # lagrima
    "A": (255, 107, 138),   # coracao
    "E": (255, 255, 255),   # brilho nos olhos
    "X": (255, 90, 70),     # exclamacao
}


def put(L, x, y, c):
    L[(x, y + OY)] = c


def pts(L, lst, c, dy=0):
    for x, y in lst:
        put(L, x, y + dy, c)


def row(L, y, x0, x1, c):
    for x in range(x0, x1 + 1):
        put(L, x, y, c)


def sym(lst):
    return list(lst) + [(27 - x, y) for x, y in lst]


HEAD_ROWS = {4: (10, 17), 5: (8, 19), 6: (7, 20), 7: (6, 21), 8: (6, 21), 9: (6, 21),
             10: (6, 21), 11: (6, 21), 12: (6, 21)}
BODY_ROWS = {12: (7, 20), 13: (6, 21), 14: (5, 22), 15: (4, 23), 16: (4, 23), 17: (4, 23),
             18: (4, 23), 19: (4, 23), 20: (4, 23), 21: (4, 23), 22: (5, 22), 23: (7, 20)}
WING = [(4, 16), (5, 16), (4, 17), (5, 17), (6, 17), (4, 18), (5, 18), (6, 18),
        (4, 19), (5, 19), (6, 19), (5, 20), (6, 20)]
HEART = [(21, 0), (23, 0), (21, 1), (22, 1), (23, 1), (22, 2)]
EXCL = [(24, 1), (25, 1), (24, 2), (25, 2), (24, 3), (25, 3), (24, 4), (25, 4),
        (24, 5), (25, 5), (24, 7), (25, 7), (24, 8), (25, 8)]
TEAR = [(19, 10), (18, 11), (19, 11), (18, 12), (19, 12)]


def build(state, frame):
    L = {}
    dy = 0
    if state == "attention" and frame == 1:
        dy = -1
    if state == "fail" and frame == 1:
        dy = 1
    fx = frame

    # patas
    for a, b in ((8, 12), (15, 19)):
        row(L, 24, a, b, "O")
        row(L, 25, a, b, "o")

    # corpo
    for y, (a, b) in BODY_ROWS.items():
        row(L, y, a, b, "Y")
    row(L, 13, 7, 20, "y")                            # sombra embaixo do queixo
    for y in range(17, 23):
        row(L, y, 9, 18, "L")                         # barriga
    wdy = -fx if state == "ok" else 0                 # asas batem quando da tudo certo
    pts(L, sym(WING), "y", wdy)

    # cabeca
    for y, (a, b) in HEAD_ROWS.items():
        row(L, y + dy, a, b, "Y")
    pts(L, [(14, 3), (14, 2), (13, 3)], "Y", dy)      # tufo
    pts(L, [(9, 6), (10, 6), (8, 7)], "L", dy)        # brilho

    # bico
    if state == "attention":
        row(L, 10 + dy, 11, 16, "O")
        row(L, 11 + dy, 10, 17, "O")
        row(L, 12 + dy, 12, 15, "K")                  # boca aberta
        row(L, 13 + dy, 11, 16, "o")
    else:
        row(L, 10 + dy, 11, 16, "O")
        row(L, 11 + dy, 10, 17, "O")
        row(L, 12 + dy, 10, 17, "o")

    # efeitos
    if state == "attention":
        pts(L, EXCL, "X", -fx)
    if state == "fail":
        pts(L, TEAR, "B", fx)
        put(L, 19, 11 + fx, "E")
    if state == "ok":
        pts(L, HEART, "A", -fx)

    # contorno automatico
    outline = {}
    for (x, y) in L:
        for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
            if (nx, ny) not in L:
                outline[(nx, ny)] = "K"
    L.update(outline)

    # rosto
    if state != "fail":
        pts(L, sym([(7, 11), (8, 11)]), "P", dy)
    if state == "info":
        if frame == 1:
            pts(L, sym([(8, 9), (9, 9)]), "K", dy)
        else:
            pts(L, sym([(8, 8), (9, 8), (8, 9), (9, 9)]), "K", dy)
    elif state == "ok":
        pts(L, sym([(7, 9), (8, 8), (9, 8), (10, 9)]), "K", dy)
    elif state == "fail":
        pts(L, sym([(8, 8), (9, 8), (8, 9), (9, 9)]), "K", dy)
        pts(L, sym([(8, 8)]), "E", dy)
        pts(L, sym([(7, 7), (8, 6), (9, 6)]), "K", dy)   # sobrancelhas tristes
    elif state == "attention":
        pts(L, sym([(8, 7), (9, 7), (8, 8), (9, 8), (8, 9), (9, 9)]), "K", dy)
        pts(L, sym([(8, 7)]), "E", dy)
    return L


def render(layer):
    img = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    px = img.load()
    for (x, y), c in layer.items():
        if 0 <= x < W and 0 <= y < H:
            px[x, y] = PAL[c] + (255,)
    return img


def up(img, s):
    return img.resize((img.width * s, img.height * s), Image.NEAREST)


def label_font(size=16):
    try:
        return ImageFont.truetype("DejaVuSans.ttf", size)
    except Exception:
        return ImageFont.load_default()


def main(out):
    pack = os.path.join(out, "pato-pixel")
    assets = os.path.join(pack, "assets")
    os.makedirs(assets, exist_ok=True)
    frames = {s: [render(build(s, f)) for f in (0, 1)] for s in STATES}

    for s in STATES:
        sheet = Image.new("RGBA", (W * 2, H), (0, 0, 0, 0))
        sheet.paste(frames[s][0], (0, 0))
        sheet.paste(frames[s][1], (W, 0))
        up(sheet, SCALE).save(os.path.join(assets, f"{s}.png"))

    manifest = {
        "schema": 1,
        "id": "pato-pixel",
        "name": "Pato Pixel",
        "author": "Quacko",
        "license": "CC0-1.0",
        "preview": "preview.png",
        "size": {"width": W * SCALE, "height": H * SCALE},
        "anchor": "bottom-left",
        "fallback": "info",
        "states": {
            s: {"type": "sprite", "file": f"assets/{s}.png",
                "frames": 2, "columns": 2, "fps": 3, "loop": True}
            for s in STATES
        },
        "phrases": {
            "pt-BR": {
                "ok": ["Seu pedido finalizou!", "Quá! Terminou!"],
                "fail": ["Eita, algo deu errado"],
                "attention": ["Preciso de você!"],
                "info": ["Aviso do pato"],
            },
            "en": {
                "ok": ["Your request finished!", "Quack! All done!"],
                "fail": ["Oops, something went wrong"],
                "attention": ["I need you!"],
                "info": ["Duck notice"],
            },
        },
    }
    with open(os.path.join(pack, "manifest.json"), "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    ps = 6
    prev = Image.new("RGBA", (W * ps * 4, H * ps), (200, 226, 244, 255))
    for i, s in enumerate(STATES):
        prev.alpha_composite(up(frames[s][0], ps), (i * W * ps, 0))
    prev.convert("RGB").save(os.path.join(pack, "preview.png"))

    ss, pad, lab = 9, 16, 28
    cell = W * ss
    bgs = [(200, 226, 244), (30, 30, 36)]
    sheet = Image.new("RGB", (pad + 4 * (cell + pad), pad + 2 * (cell + lab + pad)), (255, 255, 255))
    d = ImageDraw.Draw(sheet)
    font = label_font(16)
    for r, bg in enumerate(bgs):
        for i, s in enumerate(STATES):
            x = pad + i * (cell + pad)
            y = pad + r * (cell + lab + pad)
            tmp = Image.new("RGBA", (cell, cell), bg + (255,))
            tmp.alpha_composite(up(frames[s][0], ss))
            sheet.paste(tmp.convert("RGB"), (x, y))
            d.text((x + 4, y + cell + 6), s, fill=(60, 60, 60), font=font)
    sheet.save(os.path.join(out, "preview-pato.png"))

    gs, gpad = 7, 10
    gcell = W * gs
    gw, gh = gpad + 4 * (gcell + gpad), gcell + 2 * gpad
    gif = []
    for f in (0, 1):
        im = Image.new("RGBA", (gw, gh), (200, 226, 244, 255))
        for i, s in enumerate(STATES):
            im.alpha_composite(up(frames[s][f], gs), (gpad + i * (gcell + gpad), gpad))
        gif.append(im.convert("RGB").convert("P", palette=Image.ADAPTIVE, colors=64))
    gif[0].save(os.path.join(out, "preview-pato.gif"), save_all=True,
                append_images=gif[1:], duration=420, loop=0, disposal=2)

    zpath = os.path.join(out, "pato-pixel.quackopack")
    with zipfile.ZipFile(zpath, "w", zipfile.ZIP_DEFLATED) as z:
        z.write(os.path.join(pack, "manifest.json"), "manifest.json")
        z.write(os.path.join(pack, "preview.png"), "preview.png")
        for s in STATES:
            z.write(os.path.join(assets, f"{s}.png"), f"assets/{s}.png")
    print("ok ->", out)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "quacko-characters")
