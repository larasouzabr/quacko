import type { Mood } from "../character/types";

export type Rgb = [number, number, number];

export interface RecolorBase {
  main: Rgb;
  shade: Rgb;
  light: Rgb;
}

export interface Accessory {
  id: string;
  name: Record<string, string>;
  slot: "head";
  follows: "head";
  files: Record<Mood, string>;
  recolor: RecolorBase;
}

export interface AccessoryManifest {
  schema: number;
  id: string;
  name: Record<string, string> | string;
  slot: string;
  follows: string;
  files: Partial<Record<string, string>>;
  recolor: Record<string, unknown>;
}
