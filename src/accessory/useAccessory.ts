import { useEffect, useState } from "react";
import type { Character, Mood } from "../character/types";
import { MOODS } from "../character/types";
import { sameGrid } from "./load";
import { paletteFor, recolorPixels } from "./recolor";
import type { Accessory } from "./types";

/** Sprites recoloridos já prontos (por acessório + cor), para não refazer a cada cartão. */
const cache = new Map<string, Promise<Partial<Record<Mood, string>>>>();

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`não carregou ${url}`));
    img.src = url;
  });
}

async function build(accessory: Accessory, color: string, character: Character): Promise<Partial<Record<Mood, string>>> {
  const palette = paletteFor(color, accessory.recolor);
  const same = palette === accessory.recolor;
  const out: Partial<Record<Mood, string>> = {};
  for (const mood of MOODS) {
    const img = await loadImage(accessory.files[mood]);
    const state = character.states[mood];
    const sheet = {
      width: character.size.width * state.columns,
      height: character.size.height * Math.ceil(state.frames / state.columns),
    };
    // Mesma grade do personagem (mesmo tamanho ou múltiplo inteiro); senão, ignora.
    if (!character.pixelArt || !sameGrid({ width: img.naturalWidth, height: img.naturalHeight }, sheet)) {
      console.warn(`acessório "${accessory.id}": "${mood}" não tem a grade do personagem; ignorado`);
      continue;
    }
    if (same) {
      out[mood] = accessory.files[mood];
      continue;
    }
    // Troca exata de cor num canvas local (sem rede, sem arquivo novo).
    const canvas = document.createElement("canvas");
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    const ctx = canvas.getContext("2d");
    if (!ctx) continue;
    ctx.drawImage(img, 0, 0);
    const data = ctx.getImageData(0, 0, canvas.width, canvas.height);
    recolorPixels(data.data, accessory.recolor, palette);
    ctx.putImageData(data, 0, 0);
    out[mood] = canvas.toDataURL("image/png");
  }
  return out;
}

/** Camada do acessório pronta para o <Cat overlay>, ou undefined se desligado/inválido. */
export function useAccessoryOverlay(
  accessory: Accessory | undefined,
  color: string,
  character: Character,
): Partial<Record<Mood, string>> | undefined {
  const [overlay, setOverlay] = useState<Partial<Record<Mood, string>>>();
  useEffect(() => {
    if (!accessory) {
      setOverlay(undefined);
      return;
    }
    const key = `${accessory.id}|${color}|${character.id}`;
    let job = cache.get(key);
    if (!job) {
      job = build(accessory, color, character);
      cache.set(key, job);
    }
    let alive = true;
    job.then(
      (o) => alive && setOverlay(o),
      (e) => {
        console.warn(e);
        cache.delete(key);
      },
    );
    return () => {
      alive = false;
    };
  }, [accessory, color, character]);
  return overlay;
}
