import { describe, expect, it } from "vitest";
import { overlaySize } from "./size";

const cat = { width: 48, height: 60 };

describe("overlaySize", () => {
  it("texto normal mantém o tamanho de sempre", () => {
    expect(overlaySize(cat, 3)).toEqual(overlaySize(cat, 3, 100));
  });

  it("texto maior alarga e alonga a janela na mesma proporção", () => {
    const normal = overlaySize(cat, 3, 100);
    const larger = overlaySize(cat, 3, 150);
    expect(larger.width - normal.width).toBe(139);
    expect(larger.height).toBeGreaterThan(normal.height);
  });

  it("escala fora do intervalo fica entre 100% e 150%", () => {
    expect(overlaySize(cat, 1, 400)).toEqual(overlaySize(cat, 1, 150));
    expect(overlaySize(cat, 1, 10)).toEqual(overlaySize(cat, 1, 100));
  });
});
