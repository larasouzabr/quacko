import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { DEFAULT_ACCESSORY_ID } from "./accessory/builtin";
import { DEFAULT_CHARACTER_ID } from "./character/builtin";
import { DEFAULT_PIXEL_SIZE } from "./character/pixel";
import type { Settings } from "./types";

// Valores usados até o Rust responder (ou se o app rodar fora do Tauri).
export const DEFAULT_SETTINGS: Settings = {
  lang: "pt-BR",
  max_cards: 3,
  pixel_size: DEFAULT_PIXEL_SIZE,
  character: DEFAULT_CHARACTER_ID,
  accessory_enabled: false,
  accessory_id: DEFAULT_ACCESSORY_ID,
  accessory_color: "#ff6996",
};

/** Config do app: lida no início e atualizada quando muda no menu ou em Personalizar. */
export function useSettings(): Settings {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  useEffect(() => {
    invoke<Settings>("get_settings").then(setSettings, (e) => console.warn(e));
    const unlisten = listen<Settings>("settings-changed", ({ payload }) => setSettings(payload));
    return () => void unlisten.then((off) => off());
  }, []);
  return settings;
}
