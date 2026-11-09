import { MOODS, type Mood } from "../character/types";
import type { Accessory, AccessoryManifest, RecolorBase, Rgb } from "./types";

const ID = /^[a-z0-9][a-z0-9-]{0,39}$/;
const FILE = /^[a-z0-9][a-z0-9_-]{0,40}\.png$/i;

/**
 * Valida o accessory.json e resolve os arquivos para URLs. Acessório é só dado
 * (PNGs + JSON): nada aqui é interpretado como código, CSS ou caminho fora do pacote.
 */
export function buildAccessory(
  manifest: AccessoryManifest,
  resolveUrl: (file: string) => string | undefined,
): Accessory {
  if (manifest.schema !== 1) throw new Error(`acessório: schema ${manifest.schema} não suportado`);
  if (typeof manifest.id !== "string" || !ID.test(manifest.id)) throw new Error("acessório: id inválido");
  if (manifest.slot !== "head" || manifest.follows !== "head") {
    throw new Error(`acessório "${manifest.id}": só slot/follows "head" são suportados`);
  }
  const files = {} as Record<Mood, string>;
  for (const mood of MOODS) {
    const file = manifest.files?.[mood];
    if (typeof file !== "string" || !FILE.test(file)) {
      throw new Error(`acessório "${manifest.id}": arquivo do estado "${mood}" inválido`);
    }
    const url = resolveUrl(file);
    if (!url) throw new Error(`acessório "${manifest.id}": "${file}" não está no pacote`);
    files[mood] = url;
  }
  const name =
    typeof manifest.name === "string"
      ? { en: manifest.name }
      : Object.fromEntries(Object.entries(manifest.name ?? {}).filter(([, v]) => typeof v === "string"));
  return { id: manifest.id, name, slot: "head", follows: "head", files, recolor: parseRecolor(manifest) };
}

function rgb(value: unknown): Rgb | null {
  return Array.isArray(value) &&
    value.length === 3 &&
    value.every((c) => typeof c === "number" && Number.isInteger(c) && c >= 0 && c <= 255)
    ? (value as Rgb)
    : null;
}

function parseRecolor(manifest: AccessoryManifest): RecolorBase {
  const main = rgb(manifest.recolor?.main);
  const shade = rgb(manifest.recolor?.shade);
  const light = rgb(manifest.recolor?.light);
  if (!main || !shade || !light) {
    throw new Error(`acessório "${manifest.id}": recolor precisa de main, shade e light em [r, g, b]`);
  }
  return { main, shade, light };
}

/** O sprite do acessório tem a mesma grade do personagem (mesmo tamanho ou múltiplo inteiro)? */
export function sameGrid(
  accessory: { width: number; height: number },
  character: { width: number; height: number },
): boolean {
  if (accessory.width <= 0 || accessory.height <= 0 || character.width <= 0 || character.height <= 0) return false;
  const k = accessory.width / character.width;
  return Number.isInteger(k) && k >= 1 && k <= 16 && accessory.height === character.height * k;
}
