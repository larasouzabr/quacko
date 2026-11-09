import type { Mood } from "./character/types";

export type Source = "claude" | "agent" | "ci" | "custom";

/** Espelho de `settings::FrontendSettings` (comando get_settings e evento settings-changed). */
export interface Settings {
  lang: "pt-BR" | "en";
  max_cards: number;
  /** px por pixel de desenho do personagem pixel art (3, 4, 5, 6 ou 8). */
  pixel_size: number;
  /** Id do personagem escolhido (padrão: pato-pixel). */
  character: string;
  /** Acessório (desligado por padrão), id e cor principal #rrggbb. */
  accessory_enabled: boolean;
  accessory_id: string;
  accessory_color: string;
}

/** Espelho de `model::Notification` no Rust (evento "notify"). */
export interface Notification {
  id: number;
  source: Source;
  status: Mood;
  message: string | null;
  title: string | null;
  project: string | null;
  url: string | null;
  ttl_ms: number;
  /** Barra de progresso (ex.: checks de um PR). */
  progress?: Progress | null;
  /** Só fecha no ×: não expira (ex.: CI que terminou). */
  sticky?: boolean;
}

export interface Progress {
  done: number;
  total: number;
  failed: number;
}
