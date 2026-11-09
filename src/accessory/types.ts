import type { Mood } from "../character/types";

export type Rgb = [number, number, number];

/** As 3 cores-base que o desenho usa e que podem ser trocadas. */
export interface RecolorBase {
  main: Rgb;
  shade: Rgb;
  light: Rgb;
}

/** Acessório já validado, com os arquivos resolvidos para URLs. */
export interface Accessory {
  id: string;
  name: Record<string, string>;
  /** Onde fica: por enquanto só "head" (o desenho já acompanha a cabeça). */
  slot: "head";
  follows: "head";
  /** Um sprite sheet por estado, mesma grade e mesmos quadros do personagem. */
  files: Record<Mood, string>;
  recolor: RecolorBase;
}

/** Formato do accessory.json (schema 1). */
export interface AccessoryManifest {
  schema: number;
  id: string;
  name: Record<string, string> | string;
  slot: string;
  follows: string;
  files: Partial<Record<string, string>>;
  recolor: Record<string, unknown>;
}
