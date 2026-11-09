// Acessórios embutidos: cada pasta em src/assets/accessories/<id> (accessory.json + PNGs).
import { buildAccessory } from "./load";
import type { Accessory, AccessoryManifest } from "./types";

const manifests = import.meta.glob("../assets/accessories/*/accessory.json", {
  eager: true,
  import: "default",
}) as Record<string, AccessoryManifest>;

const files = import.meta.glob("../assets/accessories/*/*.png", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

function load(): Record<string, Accessory> {
  const all: Record<string, Accessory> = {};
  for (const [path, manifest] of Object.entries(manifests)) {
    const dir = path.slice(0, path.lastIndexOf("/") + 1);
    try {
      const accessory = buildAccessory(manifest, (file) => files[dir + file]);
      all[accessory.id] = accessory;
    } catch (e) {
      console.warn(`acessório embutido inválido em ${dir}:`, e);
    }
  }
  return all;
}

export const BUILTIN_ACCESSORIES: Readonly<Record<string, Accessory>> = load();
export const DEFAULT_ACCESSORY_ID = "laco";
