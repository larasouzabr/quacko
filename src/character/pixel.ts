export const PIXEL_SIZES = [3, 4, 5, 6, 8] as const;
export const DEFAULT_PIXEL_SIZE = 3;

function safeDpr(dpr: number): number {
  return Number.isFinite(dpr) && dpr > 0 ? dpr : 1;
}

export function cssPixel(pixelSize: number, dpr: number): number {
  const ratio = safeDpr(dpr);
  const device = Math.max(1, Math.round(pixelSize * ratio));
  return device / ratio;
}

export function snapOffset(pos: number, dpr: number): number {
  const ratio = safeDpr(dpr);
  return Math.round(pos * ratio) / ratio - pos;
}
