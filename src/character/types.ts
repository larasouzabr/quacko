export type Mood = "ok" | "fail" | "attention" | "info";
export const MOODS: readonly Mood[] = ["ok", "fail", "attention", "info"];

export const BUBBLE_PRESETS = ["pixel", "soft", "thought", "terminal"] as const;
export type BubblePreset = (typeof BUBBLE_PRESETS)[number];

export interface SpriteState {
  url: string;
  frames: number;
  columns: number;
  fps: number;
  loop: boolean;
}

export interface Character {
  id: string;
  name: string;
  author: string;
  license: string;
  pixelArt: boolean;
  size: { width: number; height: number };
  states: Record<Mood, SpriteState>;
  palette: Record<string, string>;
  bubble?: BubblePreset;
}

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
  pixel_art?: boolean;
  grid?: { width: number; height: number };
  size?: { width: number; height: number };
  bubble?: string;
  palette?: Record<string, unknown>;
  fallback?: string;
  states: Partial<Record<string, ManifestState>>;
}
