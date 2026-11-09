import type { Mood } from "./character/types";
import type { Source } from "./types";

export type Lang = "pt-BR" | "en";

interface Strings {
  defaultMessage: Record<Mood, string>;
  statusLabel: Record<Mood, string>;
  sourceLabel: Record<Source, string>;
  dismiss: string;
}

const STRINGS: Record<Lang, Strings> = {
  "pt-BR": {
    defaultMessage: {
      ok: "Seu pedido finalizou!",
      fail: "Algo deu errado por aqui",
      attention: "Preciso de você!",
      info: "Aviso do Miau",
    },
    statusLabel: { ok: "Concluído", fail: "Falhou", attention: "Atenção", info: "Aviso" },
    sourceLabel: { claude: "Claude Code", agent: "Agente", ci: "CI", custom: "Miau" },
    dismiss: "Dispensar",
  },
  en: {
    defaultMessage: {
      ok: "Your request finished!",
      fail: "Something went wrong",
      attention: "I need you!",
      info: "Miau notice",
    },
    statusLabel: { ok: "Done", fail: "Failed", attention: "Attention", info: "Notice" },
    sourceLabel: { claude: "Claude Code", agent: "Agent", ci: "CI", custom: "Miau" },
    dismiss: "Dismiss",
  },
};

export function strings(lang: Lang): Strings {
  return STRINGS[lang] ?? STRINGS["pt-BR"];
}
