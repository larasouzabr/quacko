import type { Lang } from "../i18n";

const PT = {
  title: "Personalizar",
  character: "Personagem",
  accessories: "Acessórios",
  bow: "Laço",
  on: "Ligado",
  off: "Desligado",
  color: "Cor",
  presets: { rosa: "Rosa", azul: "Azul", roxo: "Roxo", vermelho: "Vermelho" } as Record<string, string>,
  custom: "Outra cor",
  preview: "Pré-visualização",
  moods: { ok: "Concluído", fail: "Falhou", attention: "Atenção", info: "Aviso" },
  saved: "Salvo. O overlay já usa as escolhas novas.",
  hint: "A sombra e o brilho do acessório saem da cor escolhida, com contraste garantido.",
};

export type CustomizeTexts = typeof PT;

const EN: CustomizeTexts = {
  title: "Customize",
  character: "Character",
  accessories: "Accessories",
  bow: "Bow",
  on: "On",
  off: "Off",
  color: "Color",
  presets: { rosa: "Pink", azul: "Blue", roxo: "Purple", vermelho: "Red" },
  custom: "Other color",
  preview: "Preview",
  moods: { ok: "Done", fail: "Failed", attention: "Attention", info: "Notice" },
  saved: "Saved. The overlay already uses the new choices.",
  hint: "The accessory's shade and highlight come from the chosen color, with guaranteed contrast.",
};

export function customizeTexts(lang: Lang): CustomizeTexts {
  return lang === "en" ? EN : PT;
}
