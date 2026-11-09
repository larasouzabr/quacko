import { buildCharacter } from "./load";
import type { Character, CharacterManifest } from "./types";

export const DEFAULT_CHARACTER_ID = "pato-pixel";

const manifests = import.meta.glob("../assets/characters/*/manifest.json", {
  eager: true,
  import: "default",
}) as Record<string, CharacterManifest>;

const files = import.meta.glob("../assets/characters/*/assets/*.png", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

function load(): Record<string, Character> {
  const all: Record<string, Character> = {};
  for (const [path, manifest] of Object.entries(manifests)) {
    const dir = path.slice(0, path.lastIndexOf("/") + 1);
    try {
      const character = buildCharacter(manifest, (file) => files[dir + file]);
      all[character.id] = character;
    } catch (e) {
      console.warn(`personagem embutido inválido em ${dir}:`, e);
    }
  }
  return all;
}

export const BUILTIN_CHARACTERS: Readonly<Record<string, Character>> = load();

export function getCharacter(id?: string | null): Character {
  return (id && BUILTIN_CHARACTERS[id]) || BUILTIN_CHARACTERS[DEFAULT_CHARACTER_ID];
}

export const builtinCharacter = getCharacter(DEFAULT_CHARACTER_ID);
