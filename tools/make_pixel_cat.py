#!/usr/bin/env python3
"""Gera o personagem original "Gato Pixel" do Quacko (pixel art, 4 estados, 2 quadros cada).

Saidas (em ./quacko-characters):
  gato-pixel/manifest.json (schema 2), preview.png, assets/{ok,fail,attention,info}.png
  (sprite sheets em 1x: 56x28, 2 quadros de 28x28; o app amplia por CSS)
  preview-sheet.png, preview.gif, gato-pixel.quackopack (zip)

Uso: python3 make_pixel_cat.py [pasta_de_saida]
Para editar o desenho, mexa nas listas de pixels e nas funcoes build_*.
Tudo e desenhado em uma grade 28x28; o contorno escuro e gerado automaticamente.
"""
import json
import os
import sys
import zipfile

from PIL import Image, ImageDraw, ImageFont

W = H = 28            # grade do sprite
OX, OY = 2, 3         # deslocamento do desenho dentro da grade
SCALE = 1             # sprites em 1x: o app amplia por CSS em múltiplos inteiros
STATES = ["ok", "fail", "attention", "info"]

PAL = {
    "W": (255, 250, 245),   # pelo branco
    "S": (234, 223, 214),   # sombra do pelo
    "G": (232, 150, 74),    # laranja
    "g": (244, 189, 133),   # laranja claro
    "D": (196, 112, 47),    # laranja escuro (listras)
    "P": (247, 168, 176),   # rosa (bochechas, orelha)
    "K": (43, 32, 36),      # contorno e olhos
    "B": (110, 198, 240),   # gota de suor
    "Y": (255, 210, 63),    # exclamacao
    "H": (255, 107, 138),   # coracao
    "w": (255, 255, 255),   # brilho nos olhos
}


# Papéis recoloríveis (manifest "palette") -> letra do PAL
PALETTE_ROLES = {
    "fur": "W",
    "fur_shade": "S",
    "patch": "G",
    "patch_light": "g",
    "patch_dark": "D",
    "blush": "P",
    "outline": "K",
}


def hex_color(rgb):
    return "#%02x%02x%02x" % rgb


# ---------- utilitarios de desenho ----------
def put(layer, x, y, c):
    layer[(x + OX, y + OY)] = c


def pts(layer, lst, c, dy=0):
    for x, y in lst:
        put(layer, x, y + dy, c)


def row(layer, y, x0, x1, c):
    for x in range(x0, x1 + 1):
        put(layer, x, y, c)


def rect(layer, x0, y0, x1, y1, c):
    for y in range(y0, y1 + 1):
        row(layer, y, x0, x1, c)


def mirror(lst):
    return [(25 - x, y) for x, y in lst]


# ---------- formas ----------
HEAD_ROWS = {6: (7, 18), 7: (6, 19), 8: (5, 20), 9: (5, 20), 10: (5, 20),
             11: (5, 20), 12: (5, 20), 13: (5, 20), 14: (6, 19), 15: (7, 18)}

EAR_UP_L = [(6, 2), (6, 3), (7, 3), (6, 4), (7, 4), (8, 4),
            (6, 5), (7, 5), (8, 5), (9, 5)]
EAR_UP_L_PINK = [(7, 4), (7, 5)]
EAR_UP_R = mirror(EAR_UP_L)
EAR_UP_R_PINK = mirror(EAR_UP_L_PINK)

EAR_DN_L = [(6, 4), (7, 4), (8, 4), (5, 5), (6, 5), (7, 5), (8, 5), (9, 5),
            (4, 6), (5, 6), (6, 6), (4, 7), (5, 7)]
EAR_DN_L_PINK = [(6, 5), (7, 5)]
EAR_DN_R = mirror(EAR_DN_L)
EAR_DN_R_PINK = mirror(EAR_DN_L_PINK)

PATCH_ROWS = {6: (7, 11), 7: (6, 12), 8: (5, 10), 9: (5, 8)}     # mancha no topo esquerdo
PATCH_LIGHT = [(12, 6), (13, 7), (11, 8), (9, 9)]

TAIL = [
    # quadro 0: cauda levantada
    {"G": [(6, 20), (7, 20), (5, 19), (6, 19), (4, 18), (5, 18), (4, 17), (5, 17), (4, 16), (4, 15)],
     "D": [(5, 19), (4, 17)]},
    # quadro 1: cauda balancando
    {"G": [(5, 20), (6, 20), (7, 20), (4, 19), (5, 19), (3, 18), (4, 18), (3, 17), (3, 16)],
     "D": [(4, 19), (3, 17)]},
]

DROP = [(24, 7), (23, 8), (24, 8), (23, 9), (24, 9)]
HEART = [(22, 3), (24, 3), (22, 4), (23, 4), (24, 4), (23, 5)]


# ---------- montagem de cada quadro ----------
def build(state, frame):
    L = {}
    dy = 0                                   # deslocamento vertical da cabeca
    if state == "attention" and frame == 1:
        dy = -1
    if state == "fail" and frame == 1:
        dy = 1
    fx = frame                               # deslocamento dos efeitos (0 ou 1)

    # cauda
    t = TAIL[frame]
    pts(L, t["G"], "G")
    pts(L, t["D"], "D")

    # corpo
    rect(L, 8, 16, 17, 20, "W")
    row(L, 16, 8, 17, "S")                   # sombra do pescoco
    row(L, 21, 8, 11, "W")                   # patinhas
    row(L, 21, 14, 17, "W")
    for y, (a, b) in {17: (15, 17), 18: (14, 17), 19: (15, 17)}.items():
        row(L, y, a, b, "G")                 # mancha no corpo

    # orelhas
    if state == "fail":
        pts(L, EAR_DN_L, "G", dy)
        pts(L, EAR_DN_L_PINK, "P", dy)
        pts(L, EAR_DN_R, "W", dy)
        pts(L, EAR_DN_R_PINK, "P", dy)
    else:
        pts(L, EAR_UP_L, "G", dy)
        pts(L, EAR_UP_L_PINK, "P", dy)
        pts(L, EAR_UP_R, "W", dy)
        pts(L, EAR_UP_R_PINK, "P", dy)

    # cabeca
    for y, (a, b) in HEAD_ROWS.items():
        row(L, y + dy, a, b, "W")
    for y, (a, b) in PATCH_ROWS.items():
        row(L, y + dy, a, b, "G")
    pts(L, PATCH_LIGHT, "g", dy)

    # efeitos (entram no contorno)
    if state == "attention":
        rect(L, 21, 0 - fx, 22, 3 - fx, "Y")
        rect(L, 21, 5 - fx, 22, 6 - fx, "Y")
    if state == "fail":
        pts(L, DROP, "B", fx)
        put(L, 23, 9 + fx, "w")                 # brilho da gota
    if state == "ok":
        pts(L, HEART, "H", -fx)

    # contorno automatico (vizinhos de 4 direcoes)
    outline = {}
    for (x, y) in L:
        for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
            if (nx, ny) not in L:
                outline[(nx, ny)] = "K"
    L.update(outline)

    # rosto (por cima de tudo)
    face_dy = dy
    if state != "fail":
        pts(L, [(6, 12), (7, 12), (18, 12), (19, 12)], "P", face_dy)   # bochechas

    if state == "info":
        if frame == 1:   # piscando
            pts(L, [(8, 11), (9, 11), (16, 11), (17, 11)], "K", face_dy)
        else:
            pts(L, [(8, 10), (9, 10), (8, 11), (9, 11),
                    (16, 10), (17, 10), (16, 11), (17, 11)], "K", face_dy)
        pts(L, [(11, 12), (12, 13), (13, 13), (14, 12)], "K", face_dy)
    elif state == "ok":
        pts(L, [(8, 11), (9, 10), (10, 11), (15, 11), (16, 10), (17, 11)], "K", face_dy)
        pts(L, [(11, 12), (14, 12), (12, 13), (13, 13)], "K", face_dy)
        pts(L, [(12, 12), (13, 12)], "P", face_dy)
    elif state == "fail":
        pts(L, [(8, 10), (9, 10), (8, 11), (9, 11),
                (16, 10), (17, 10), (16, 11), (17, 11)], "K", face_dy)
        pts(L, [(8, 10), (16, 10)], "w", face_dy)                       # olhos marejados
        pts(L, [(7, 8), (8, 8), (9, 7), (18, 8), (17, 8), (16, 7)], "K", face_dy)  # sobrancelhas
        pts(L, [(11, 13), (12, 12), (13, 12), (14, 13)], "K", face_dy)  # boca triste
    elif state == "attention":
        pts(L, [(8, 9), (9, 9), (8, 10), (9, 10), (8, 11), (9, 11),
                (16, 9), (17, 9), (16, 10), (17, 10), (16, 11), (17, 11)], "K", face_dy)
        pts(L, [(8, 9), (16, 9)], "w", face_dy)
        pts(L, [(12, 12), (13, 12), (12, 13), (13, 13)], "K", face_dy)
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
    pack = os.path.join(out, "gato-pixel")
    assets = os.path.join(pack, "assets")
    os.makedirs(assets, exist_ok=True)

    frames = {s: [render(build(s, f)) for f in (0, 1)] for s in STATES}

    # sprite sheets (2 quadros lado a lado)
    for s in STATES:
        sheet = Image.new("RGBA", (W * 2, H), (0, 0, 0, 0))
        sheet.paste(frames[s][0], (0, 0))
        sheet.paste(frames[s][1], (W, 0))
        up(sheet, SCALE).save(os.path.join(assets, f"{s}.png"))

    # manifest no formato .quackopack (schema 2, quacko-spec-personalizacao.md)
    manifest = {
        "schema": 2,
        "id": "gato-pixel",
        "name": "Gato Pixel",
        "author": "Quacko",
        "license": "CC0-1.0",
        "preview": "preview.png",
        "pixel_art": True,
        "grid": {"width": W, "height": H},
        "bubble": "pixel",
        "anchor": "bottom-left",
        "fallback": "info",
        "states": {
            s: {"type": "sprite", "file": f"assets/{s}.png",
                "frames": 2, "columns": 2, "fps": 3, "loop": True}
            for s in STATES
        },
        # Cores exatas usadas nos PNGs, por papel: permite recolorir sem arte nova.
        # Efeitos (coração, exclamação, gota) e brilho dos olhos ficam fora: cores fixas.
        "palette": {role: hex_color(PAL[key]) for role, key in PALETTE_ROLES.items()},
        "phrases": {
            "pt-BR": {
                "ok": ["Seu pedido finalizou!", "Prontinho!"],
                "fail": ["Algo deu errado por aqui"],
                "attention": ["Preciso de você!"],
                "info": ["Aviso do Quacko"],
            },
            "en": {
                "ok": ["Your request finished!", "All done!"],
                "fail": ["Something went wrong"],
                "attention": ["I need you!"],
                "info": ["Quacko notice"],
            },
        },
    }
    with open(os.path.join(pack, "manifest.json"), "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    # preview do pacote (4 estados, quadro 0)
    ps = 6
    prev = Image.new("RGBA", (W * ps * 4, H * ps), (251, 214, 230, 255))
    for i, s in enumerate(STATES):
        prev.alpha_composite(up(frames[s][0], ps), (i * W * ps, 0))
    prev.convert("RGB").save(os.path.join(pack, "preview.png"))

    # folha de visualizacao: fundo rosa e fundo escuro
    ss, pad, lab = 9, 16, 28
    cell = W * ss
    bgs = [(251, 214, 230), (30, 30, 36)]
    sheet = Image.new("RGB", (pad + 4 * (cell + pad), pad + 2 * (cell + lab + pad)), (255, 255, 255))
    d = ImageDraw.Draw(sheet)
    font = label_font(16)
    for r, bg in enumerate(bgs):
        for i, s in enumerate(STATES):
            x = pad + i * (cell + pad)
            y = pad + r * (cell + lab + pad)
            d.rectangle([x, y, x + cell, y + cell], fill=bg)
            tmp = Image.new("RGBA", (cell, cell), bg + (255,))
            tmp.alpha_composite(up(frames[s][0], ss))
            sheet.paste(tmp.convert("RGB"), (x, y))
            d.text((x + 4, y + cell + 6), s, fill=(60, 60, 60), font=font)
    sheet.save(os.path.join(out, "preview-sheet.png"))

    # GIF animado com os 4 estados
    gs, gpad = 7, 10
    gcell = W * gs
    gw = gpad + 4 * (gcell + gpad)
    gh = gcell + 2 * gpad
    gif = []
    for f in (0, 1):
        im = Image.new("RGBA", (gw, gh), (251, 214, 230, 255))
        for i, s in enumerate(STATES):
            im.alpha_composite(up(frames[s][f], gs), (gpad + i * (gcell + gpad), gpad))
        gif.append(im.convert("RGB").convert("P", palette=Image.ADAPTIVE, colors=64))
    gif[0].save(os.path.join(out, "preview.gif"), save_all=True, append_images=gif[1:],
                duration=420, loop=0, disposal=2)

    # .quackopack (zip com manifest na raiz)
    zpath = os.path.join(out, "gato-pixel.quackopack")
    with zipfile.ZipFile(zpath, "w", zipfile.ZIP_DEFLATED) as z:
        z.write(os.path.join(pack, "manifest.json"), "manifest.json")
        z.write(os.path.join(pack, "preview.png"), "preview.png")
        for s in STATES:
            z.write(os.path.join(assets, f"{s}.png"), f"assets/{s}.png")

    print("ok ->", out)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "quacko-characters")
