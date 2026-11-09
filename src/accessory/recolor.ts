import type { RecolorBase, Rgb } from "./types";

export function hexToRgb(hex: string): Rgb | null {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex.trim());
  return m ? [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)] : null;
}

export function rgbToHex([r, g, b]: Rgb): string {
  return "#" + [r, g, b].map((c) => c.toString(16).padStart(2, "0")).join("");
}

/** Luminância relativa (WCAG). */
function luminance([r, g, b]: Rgb): number {
  const lin = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

export function contrastRatio(a: Rgb, b: Rgb): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

function mix(a: Rgb, b: Rgb, t: number): Rgb {
  return [0, 1, 2].map((i) => Math.round(a[i] + (b[i] - a[i]) * t)) as Rgb;
}

/** Contorno do personagem (não é recolorido): o acessório não pode sumir contra ele. */
export const OUTLINE: Rgb = [43, 32, 36];
const MIN_OUTLINE_CONTRAST = 3;
const MIN_STEP_CONTRAST = 1.25;

/**
 * Sombra e luz a partir da cor escolhida, na mesma proporção da paleta original:
 * a sombra escurece a cor e a luz clareia, mantendo contraste entre elas e contra o
 * contorno do personagem.
 */
export function deriveShades(main: Rgb, base: RecolorBase): RecolorBase {
  let m = main;
  // Cor escura demais some contra o contorno: clareia até ter contraste.
  for (let i = 0; i < 20 && contrastRatio(m, OUTLINE) < MIN_OUTLINE_CONTRAST; i++) {
    m = mix(m, [255, 255, 255], 0.1);
  }
  // Quanto a paleta original escurece/clareia (média por canal), aplicado à cor nova.
  const darken = 1 - (base.shade[0] + base.shade[1] + base.shade[2]) / (base.main[0] + base.main[1] + base.main[2]);
  const lighten =
    (base.light[0] + base.light[1] + base.light[2] - (base.main[0] + base.main[1] + base.main[2])) /
    Math.max(1, 765 - (base.main[0] + base.main[1] + base.main[2]));
  let shade = mix(m, [0, 0, 0], Math.max(0.15, darken));
  let light = mix(m, [255, 255, 255], Math.max(0.3, lighten));
  for (let i = 0; i < 10 && contrastRatio(m, shade) < MIN_STEP_CONTRAST; i++) shade = mix(shade, [0, 0, 0], 0.15);
  for (let i = 0; i < 10 && contrastRatio(light, m) < MIN_STEP_CONTRAST; i++) light = mix(light, [255, 255, 255], 0.2);
  return { main: m, shade, light };
}

/** Cores prontas. "rosa" é exatamente a paleta original do laço. */
export const PRESETS: Record<string, RecolorBase> = {
  rosa: { main: [255, 105, 150], shade: [214, 64, 110], light: [255, 176, 205] },
  azul: { main: [74, 144, 226], shade: [44, 98, 168], light: [160, 202, 245] },
  roxo: { main: [156, 98, 220], shade: [110, 62, 168], light: [208, 176, 242] },
  vermelho: { main: [230, 57, 70], shade: [170, 32, 44], light: [246, 150, 156] },
};

/** Paleta final para uma cor escolhida: a de um preset, se for a cor dele; senão derivada. */
export function paletteFor(color: string, base: RecolorBase): RecolorBase {
  const rgb = hexToRgb(color) ?? base.main;
  for (const preset of Object.values(PRESETS)) {
    if (preset.main.every((c, i) => c === rgb[i])) return preset;
  }
  if (base.main.every((c, i) => c === rgb[i])) return base;
  return deriveShades(rgb, base);
}

/**
 * Troca exata de cor, pixel a pixel (RGBA): só pixels visíveis com uma das cores-base
 * mudam; alfa, contorno e qualquer outra cor ficam como estão.
 */
export function recolorPixels(data: Uint8ClampedArray | Uint8Array, from: RecolorBase, to: RecolorBase): void {
  const pairs: [Rgb, Rgb][] = [
    [from.main, to.main],
    [from.shade, to.shade],
    [from.light, to.light],
  ];
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] === 0) continue;
    for (const [a, b] of pairs) {
      if (data[i] === a[0] && data[i + 1] === a[1] && data[i + 2] === a[2]) {
        data[i] = b[0];
        data[i + 1] = b[1];
        data[i + 2] = b[2];
        break;
      }
    }
  }
}
