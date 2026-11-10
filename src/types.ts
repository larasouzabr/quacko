import type { Mood } from "./character/types";

export type Source = "claude" | "agent" | "ci" | "custom";

export interface Settings {
  lang: "pt-BR" | "en";
  max_cards: number;
  pixel_size: number;
  text_scale: number;
  character: string;
  accessory_enabled: boolean;
  accessory_id: string;
  accessory_color: string;
}

export interface Notification {
  id: number;
  source: Source;
  status: Mood;
  message: string | null;
  title: string | null;
  project: string | null;
  url: string | null;
  ttl_ms: number;
  progress?: Progress | null;
  sticky?: boolean;
}

export interface Progress {
  done: number;
  total: number;
  failed: number;
}
