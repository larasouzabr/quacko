// Personagem padrão embutido: "Gato Pixel" (src/assets/characters/gato-pixel).
// Para regenerar os sprites, veja a seção 8 do miau-spec.md.
import manifest from "../assets/characters/gato-pixel/manifest.json";
import { buildCharacter } from "./load";
import type { CharacterManifest } from "./types";

const PACK_DIR = "../assets/characters/gato-pixel/";

const files = import.meta.glob("../assets/characters/gato-pixel/assets/*.png", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

export const builtinCharacter = buildCharacter(
  manifest as CharacterManifest,
  (file) => files[PACK_DIR + file],
);
