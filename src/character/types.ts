/** Os 4 estados do personagem; batem com o `status` das notificações. */
export type Mood = "ok" | "fail" | "attention" | "info";
export const MOODS: readonly Mood[] = ["ok", "fail", "attention", "info"];

/** Presets de balão conhecidos (quacko-spec-personalizacao.md, seção 3). */
export const BUBBLE_PRESETS = ["pixel", "soft", "thought", "terminal"] as const;
export type BubblePreset = (typeof BUBBLE_PRESETS)[number];

/** Um estado pronto para desenhar: sprite sheet com `frames` quadros em `columns` colunas. */
export interface SpriteState {
  url: string;
  frames: number;
  columns: number;
  fps: number;
  loop: boolean;
}

/** Personagem já validado, com os arquivos resolvidos para URLs. */
export interface Character {
  id: string;
  name: string;
  author: string;
  license: string;
  /** Pixel art: ampliado por CSS em múltiplos inteiros, com `image-rendering: pixelated`. */
  pixelArt: boolean;
  /** Pixel art: tamanho de 1 quadro em pixels de desenho (a "grade").
   * Demais: tamanho de 1 quadro em px CSS, a 100%. */
  size: { width: number; height: number };
  states: Record<Mood, SpriteState>;
  /** Papéis de cor recoloríveis (só pixel art), em `#rrggbb` ou `#rrggbbaa`. */
  palette: Record<string, string>;
  /** Preset de balão sugerido pelo personagem (só sugestão). */
  bubble?: BubblePreset;
}

/** Formato do `manifest.json` de um `.quackopack` (schema 1 ou 2), só os campos usados. */
export interface ManifestState {
  type: string;
  file: string;
  frames?: number;
  columns?: number;
  fps?: number;
  loop?: boolean;
}

export interface CharacterManifest {
  schema: number;
  id: string;
  name: string;
  author: string;
  license: string;
  /** schema 2 */
  pixel_art?: boolean;
  /** schema 2, pixel art */
  grid?: { width: number; height: number };
  /** schema 1, ou schema 2 sem pixel art */
  size?: { width: number; height: number };
  bubble?: string;
  palette?: Record<string, unknown>;
  fallback?: string;
  states: Partial<Record<string, ManifestState>>;
}
