import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { PNG } from "pngjs";
import { describe, expect, it } from "vitest";
import { MOODS, type Mood } from "../character/types";
import { sameGrid } from "./load";
import { contrastRatio, deriveShades, OUTLINE, paletteFor, PRESETS, recolorPixels } from "./recolor";
import type { RecolorBase, Rgb } from "./types";

const ROOT = resolve(__dirname, "../..");
const read = (path: string) => PNG.sync.read(readFileSync(resolve(ROOT, path)));
const duck = (m: Mood) => read(`src/assets/characters/pato-pixel/assets/${m}.png`);
const bow = (m: Mood) => read(`src/assets/accessories/laco/${m}.png`);
const reference = (m: Mood) => read(`src/test-fixtures/pato-pixel-laco/${m}.png`);
const BASE: RecolorBase = PRESETS.rosa;
const SCALE = 6;
const FRAME = 28;

function compose(base: PNG, top: PNG): PNG {
  const out = new PNG({ width: base.width, height: base.height });
  for (let i = 0; i < base.data.length; i += 4) {
    const a = top.data[i + 3] / 255;
    for (let c = 0; c < 3; c++) out.data[i + c] = Math.round(top.data[i + c] * a + base.data[i + c] * (1 - a));
    out.data[i + 3] = Math.max(top.data[i + 3], base.data[i + 3]);
  }
  return out;
}

const pixel = (png: PNG, x: number, y: number) => {
  const i = (y * png.width + x) * 4;
  return [png.data[i], png.data[i + 1], png.data[i + 2], png.data[i + 3]];
};

function diff(composed: PNG, ref: PNG): { x: number; y: number }[] {
  const out: { x: number; y: number }[] = [];
  for (let y = 0; y < composed.height; y++) {
    for (let x = 0; x < composed.width; x++) {
      const want = pixel(composed, x, y);
      let same = true;
      for (let dy = 0; dy < SCALE && same; dy++) {
        for (let dx = 0; dx < SCALE && same; dx++) {
          const got = pixel(ref, x * SCALE + dx, y * SCALE + dy);
          same = got.every((v, k) => v === want[k]);
        }
      }
      if (!same) out.push({ x, y });
    }
  }
  return out;
}

describe("sobreposição do laço x pato-pixel-laco (cor padrão)", () => {
  it("referência está em 6x e o pato/laço em 1x, mesma grade", () => {
    for (const m of MOODS) {
      expect([reference(m).width, reference(m).height], m).toEqual([duck(m).width * SCALE, duck(m).height * SCALE]);
      expect([bow(m).width, bow(m).height], m).toEqual([duck(m).width, duck(m).height]);
    }
  });

  for (const m of ["fail", "attention", "info"] as Mood[]) {
    it(`${m}: idêntico em pixels, nos 2 quadros`, () => {
      expect(diff(compose(duck(m), bow(m)), reference(m))).toEqual([]);
    });
  }

  it("ok: o laço é idêntico; a única diferença é o coraçãozinho, que a referência moveu", () => {
    const composed = compose(duck("ok"), bow("ok"));
    const bowPng = bow("ok");
    const diffs = diff(composed, reference("ok"));
    expect(diffs.filter(({ x, y }) => pixel(bowPng, x, y)[3] > 0)).toEqual([]);
    const heartColors = new Set(["255,107,138", "43,32,36"]);
    for (const { x, y } of diffs) {
      expect(x % FRAME, `x=${x}`).toBeGreaterThanOrEqual(20);
      expect(y, `y=${y}`).toBeLessThanOrEqual(5);
      for (const px of [pixel(composed, x, y), pixel(reference("ok"), x * SCALE, y * SCALE)]) {
        if (px[3] > 0) expect(heartColors.has(px.slice(0, 3).join(","))).toBe(true);
      }
    }
    expect(diffs.length).toBeGreaterThan(0);
  });
});

describe("recoloração do laço", () => {
  it("a cor padrão é a paleta original: nada muda", () => {
    expect(paletteFor("#ff6996", BASE)).toBe(BASE);
    for (const m of MOODS) {
      const png = bow(m);
      const before = Buffer.from(png.data);
      recolorPixels(png.data, BASE, paletteFor("#ff6996", BASE));
      expect(Buffer.from(png.data).equals(before), m).toBe(true);
    }
  });

  it("preset azul troca exatamente as 3 cores-base; contorno e transparência ficam", () => {
    for (const m of MOODS) {
      const original = bow(m);
      const png = bow(m);
      recolorPixels(png.data, BASE, PRESETS.azul);
      let changed = 0;
      for (let i = 0; i < png.data.length; i += 4) {
        const before = [...original.data.subarray(i, i + 4)];
        const after = [...png.data.subarray(i, i + 4)];
        expect(after[3]).toBe(before[3]);
        if (before[3] === 0) continue;
        const idx = (["main", "shade", "light"] as const).findIndex((k) => BASE[k].every((c, j) => c === before[j]));
        if (idx >= 0) {
          const key = (["main", "shade", "light"] as const)[idx];
          expect(after.slice(0, 3)).toEqual(PRESETS.azul[key]);
          changed++;
        } else {
          expect(after).toEqual(before);
        }
      }
      expect(changed, m).toBeGreaterThan(0);
    }
  });

  it("cor escolhida livremente: sombra mais escura, luz mais clara, com contraste", () => {
    for (const hex of ["#00a0a0", "#ffd400", "#7a7a7a", "#ffffff"]) {
      const p = paletteFor(hex, BASE);
      expect(contrastRatio(p.main, p.shade), hex).toBeGreaterThanOrEqual(1.25);
      expect(contrastRatio(p.light, p.main), hex).toBeGreaterThanOrEqual(1.2);
      expect(contrastRatio(p.main, OUTLINE), hex).toBeGreaterThanOrEqual(3);
    }
  });

  it("cor escura demais é clareada para não sumir contra o contorno", () => {
    const dark: Rgb = [16, 16, 16];
    const p = deriveShades(dark, BASE);
    expect(contrastRatio(p.main, OUTLINE)).toBeGreaterThanOrEqual(3);
    expect(p.main).not.toEqual(dark);
  });

  it("cores prontas: rosa é a original e todas têm contraste com o contorno", () => {
    expect(PRESETS.rosa).toEqual({ main: [255, 105, 150], shade: [214, 64, 110], light: [255, 176, 205] });
    for (const [name, p] of Object.entries(PRESETS)) {
      expect(contrastRatio(p.main, OUTLINE), name).toBeGreaterThanOrEqual(3);
      expect(contrastRatio(p.main, p.shade), name).toBeGreaterThan(1.2);
    }
  });
});

describe("acessório com tamanho inválido", () => {
  const sheet = { width: 56, height: 28 };
  it("aceita a mesma grade (1x) ou um múltiplo inteiro (6x)", () => {
    expect(sameGrid({ width: 56, height: 28 }, sheet)).toBe(true);
    expect(sameGrid({ width: 336, height: 168 }, sheet)).toBe(true);
  });
  it("recusa tamanho fora da grade", () => {
    for (const bad of [
      { width: 57, height: 28 },
      { width: 56, height: 29 },
      { width: 112, height: 28 },
      { width: 0, height: 0 },
      { width: 56 * 17, height: 28 * 17 },
    ]) {
      expect(sameGrid(bad, sheet), JSON.stringify(bad)).toBe(false);
    }
  });
});
