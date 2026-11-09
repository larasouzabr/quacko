import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../types";

export interface Appearance {
  character: string;
  accessoryEnabled: boolean;
  accessoryColor: string;
}

/** De onde a tela Personalizar lê e para onde grava (trocável na pré-visualização). */
export interface CustomizeApi {
  settings(): Promise<Settings>;
  save(a: Appearance): Promise<Settings>;
}

export const tauriCustomizeApi: CustomizeApi = {
  settings: () => invoke<Settings>("get_settings"),
  save: (a) =>
    invoke<Settings>("set_appearance", {
      character: a.character,
      accessoryEnabled: a.accessoryEnabled,
      accessoryColor: a.accessoryColor,
    }),
};
