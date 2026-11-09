// Tamanho do pixel do pixel art (quacko-spec-personalizacao.md, seção 4).

/** Valores oferecidos: px por pixel de desenho. Sempre inteiros. */
export const PIXEL_SIZES = [3, 4, 5, 6, 8] as const;
export const DEFAULT_PIXEL_SIZE = 3;

function safeDpr(dpr: number): number {
  return Number.isFinite(dpr) && dpr > 0 ? dpr : 1;
}

/**
 * Tamanho, em px CSS, de 1 pixel do desenho. Arredonda `pixelSize × dpr` para um número
 * inteiro de pixels físicos: em telas com escala 125% ou 150%, isso evita pixels do
 * desenho com tamanhos diferentes.
 */
export function cssPixel(pixelSize: number, dpr: number): number {
  const ratio = safeDpr(dpr);
  const device = Math.max(1, Math.round(pixelSize * ratio));
  return device / ratio;
}

/** Quanto mover (px CSS) para `pos` cair exatamente num pixel físico. */
export function snapOffset(pos: number, dpr: number): number {
  const ratio = safeDpr(dpr);
  return Math.round(pos * ratio) / ratio - pos;
}
