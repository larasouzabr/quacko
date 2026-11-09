import {
  BUBBLE_PRESETS,
  MOODS,
  type BubblePreset,
  type Character,
  type CharacterManifest,
  type ManifestState,
  type Mood,
  type SpriteState,
} from "./types";

/** Limites da quacko-spec-personalizacao.md, seção 1. */
const MAX_DIMENSION = 1024;
const MAX_GRID = 128;
const MAX_FPS = 30;
const COLOR = /^#[0-9a-f]{6}([0-9a-f]{2})?$/i;
const ROLE = /^[a-z][a-z0-9_]{0,31}$/;

/**
 * Monta um personagem a partir do manifest (schema 1 ou 2). `resolveUrl` traduz o
 * caminho do arquivo dentro do pacote (ex.: "assets/ok.png") para uma URL que o
 * webview consegue carregar.
 *
 * schema 1 (`size` em px) é lido como personagem não pixel art, como manda a spec.
 * Por enquanto só os tipos "sprite" e "image" são suportados; os demais entram junto
 * com a importação de pacotes.
 */
export function buildCharacter(
  manifest: CharacterManifest,
  resolveUrl: (file: string) => string | undefined,
): Character {
  if (manifest.schema !== 1 && manifest.schema !== 2) {
    throw new Error(`personagem "${manifest.id}": schema ${manifest.schema} não suportado`);
  }
  const pixelArt = manifest.schema === 2 && manifest.pixel_art === true;
  const size = pixelArt
    ? box(manifest.grid, MAX_GRID, "grid", true)
    : box(manifest.size, MAX_DIMENSION, "size", false);

  const parsed: Partial<Record<Mood, SpriteState>> = {};
  for (const mood of MOODS) {
    const def = manifest.states?.[mood];
    if (def) parsed[mood] = parseState(mood, def, resolveUrl);
  }

  const fallbackMood = MOODS.find((m) => m === manifest.fallback);
  const fallback = fallbackMood ? parsed[fallbackMood] : undefined;

  const states = {} as Record<Mood, SpriteState>;
  for (const mood of MOODS) {
    const state = parsed[mood] ?? fallback;
    if (!state) {
      throw new Error(`personagem "${manifest.id}": estado "${mood}" ausente e sem fallback válido`);
    }
    states[mood] = state;
  }

  return {
    id: manifest.id,
    name: manifest.name,
    author: manifest.author,
    license: manifest.license,
    pixelArt,
    size,
    states,
    palette: pixelArt ? parsePalette(manifest.palette) : {},
    bubble: BUBBLE_PRESETS.find((p): p is BubblePreset => p === manifest.bubble),
  };
}

function parseState(
  mood: Mood,
  def: ManifestState,
  resolveUrl: (file: string) => string | undefined,
): SpriteState {
  if (def.type !== "sprite" && def.type !== "image") {
    throw new Error(`estado "${mood}": tipo "${def.type}" ainda não suportado`);
  }
  const url = resolveUrl(def.file);
  if (!url) {
    throw new Error(`estado "${mood}": arquivo "${def.file}" não encontrado no pacote`);
  }
  if (def.type === "image") {
    return { url, frames: 1, columns: 1, fps: 1, loop: false };
  }

  const frames = positiveInt(def.frames, 1);
  const columns = positiveInt(def.columns, frames);
  const fps = Math.min(positiveInt(def.fps, 1), MAX_FPS);
  // A animação CSS percorre uma única linha do sprite. Se os quadros não couberem
  // numa linha, mostra só o primeiro em vez de desenhar quadros errados.
  const animatable = frames > 1 && columns >= frames;
  return {
    url,
    frames: animatable ? frames : 1,
    columns,
    fps,
    loop: def.loop ?? true,
  };
}

/** Só papéis com nome simples e cor `#rrggbb`/`#rrggbbaa`; o resto é ignorado. */
function parsePalette(raw: unknown): Record<string, string> {
  const palette: Record<string, string> = {};
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return palette;
  for (const [role, value] of Object.entries(raw)) {
    if (ROLE.test(role) && typeof value === "string" && COLOR.test(value)) {
      palette[role] = value.toLowerCase();
    }
  }
  return palette;
}

function positiveInt(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : fallback;
}

function box(
  raw: { width: number; height: number } | undefined,
  max: number,
  field: string,
  integer: boolean,
): { width: number; height: number } {
  const ok = (v: unknown): v is number =>
    typeof v === "number" && Number.isFinite(v) && v > 0 && v <= max && (!integer || Number.isInteger(v));
  if (!raw || !ok(raw.width) || !ok(raw.height)) {
    throw new Error(`campo ${field} ausente ou inválido (máximo ${max})`);
  }
  return { width: raw.width, height: raw.height };
}
