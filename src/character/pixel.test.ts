import { describe, expect, it } from "vitest";
import { cssPixel, PIXEL_SIZES, snapOffset } from "./pixel";

const DPRS = [1, 1.25, 1.5, 1.75, 2, 2.25, 3];

describe("tamanho do pixel", () => {
  it("em telas 1x e 2x fica exatamente o valor escolhido", () => {
    for (const size of PIXEL_SIZES) {
      expect(cssPixel(size, 1)).toBe(size);
      expect(cssPixel(size, 2)).toBe(size);
    }
  });

  it("sempre vira um número inteiro de pixels físicos", () => {
    for (const dpr of DPRS) {
      for (const size of PIXEL_SIZES) {
        const device = cssPixel(size, dpr) * dpr;
        expect(Math.abs(device - Math.round(device)), `${size}px @ ${dpr}x`).toBeLessThan(1e-9);
        expect(device).toBeGreaterThanOrEqual(1);
      }
    }
  });

  it("um quadro inteiro (28 pixels de desenho) também cai em pixels físicos inteiros", () => {
    for (const dpr of DPRS) {
      const device = 28 * cssPixel(3, dpr) * dpr;
      expect(Math.abs(device - Math.round(device)), `@ ${dpr}x`).toBeLessThan(1e-9);
    }
  });

  it("125% e 150% arredondam para o pixel físico mais próximo", () => {
    expect(cssPixel(3, 1.25) * 1.25).toBeCloseTo(4);
    expect(cssPixel(4, 1.5) * 1.5).toBeCloseTo(6);
  });

  it("dpr inválido é tratado como 1", () => {
    expect(cssPixel(3, 0)).toBe(3);
    expect(cssPixel(3, Number.NaN)).toBe(3);
  });
});

describe("alinhamento ao pixel físico", () => {
  it("posição já alinhada não move", () => {
    expect(snapOffset(10, 2)).toBe(0);
    expect(snapOffset(10.5, 2)).toBe(0);
  });

  it("posição fracionária move para o pixel físico mais próximo", () => {
    for (const dpr of DPRS) {
      for (const pos of [0.3, 7.77, 101.49]) {
        const snapped = (pos + snapOffset(pos, dpr)) * dpr;
        expect(Math.abs(snapped - Math.round(snapped))).toBeLessThan(1e-9);
        expect(Math.abs(snapOffset(pos, dpr))).toBeLessThanOrEqual(0.5 / dpr + 1e-9);
      }
    }
  });
});
