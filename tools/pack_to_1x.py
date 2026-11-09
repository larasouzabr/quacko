#!/usr/bin/env python3
"""Converte sprites de pixel art ampliados (ex.: 6x, schema 1) para 1x, sem perda.

Recusa a conversão se algum bloco de SCALE x SCALE não tiver uma cor só: nesse caso a
imagem não é pixel art ampliada por inteiro e converter perderia informação.

Uso:
  python3 tools/pack_to_1x.py personagem PASTA_DO_PACOTE_6X PASTA_SAIDA [escala]
      -> manifest schema 2 (pixel_art, grid) + assets/*.png em 1x
  python3 tools/pack_to_1x.py acessorio PASTA_DO_ACESSORIO_6X PASTA_SAIDA [escala]
      -> accessory.json igual + PNGs em 1x
(precisa do Pillow; ver seção 8 do quacko-spec.md para o venv)
"""
import json
import os
import shutil
import sys

from PIL import Image


def to_1x(src, dst, scale):
    img = Image.open(src).convert("RGBA")
    w, h = img.size
    if w % scale or h % scale:
        sys.exit(f"{src}: {w}x{h} não é múltiplo de {scale}")
    small = img.resize((w // scale, h // scale), Image.NEAREST)
    # Sem perda: ampliar de volta tem que dar exatamente a original.
    if small.resize((w, h), Image.NEAREST).tobytes() != img.tobytes():
        sys.exit(f"{src}: não é pixel art {scale}x por inteiro (blocos com mais de uma cor)")
    small.save(dst)
    return small.size


def character(src, out, scale):
    m = json.load(open(os.path.join(src, "manifest.json"), encoding="utf-8"))
    os.makedirs(os.path.join(out, "assets"), exist_ok=True)
    frame = None
    for state in m["states"].values():
        size = to_1x(os.path.join(src, state["file"]), os.path.join(out, state["file"]), scale)
        frame = (size[0] // state.get("columns", 1), size[1])
    m["schema"] = 2
    m["pixel_art"] = True
    m["grid"] = {"width": frame[0], "height": frame[1]}
    m.pop("size", None)
    m.setdefault("bubble", "pixel")
    with open(os.path.join(out, "manifest.json"), "w", encoding="utf-8") as f:
        json.dump(m, f, ensure_ascii=False, indent=2)
    if os.path.exists(os.path.join(src, m.get("preview", "preview.png"))):
        shutil.copy(os.path.join(src, m.get("preview", "preview.png")), out)


def accessory(src, out, scale):
    m = json.load(open(os.path.join(src, "accessory.json"), encoding="utf-8"))
    os.makedirs(out, exist_ok=True)
    for f in m["files"].values():
        to_1x(os.path.join(src, f), os.path.join(out, f), scale)
    shutil.copy(os.path.join(src, "accessory.json"), out)


if __name__ == "__main__":
    kind, src, out = sys.argv[1:4]
    scale = int(sys.argv[4]) if len(sys.argv) > 4 else 6
    {"personagem": character, "acessorio": accessory}[kind](src, out, scale)
    print("ok ->", out)
