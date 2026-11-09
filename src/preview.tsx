// Pré-visualização dos cartões com dados de exemplo, fora do Tauri.
// Uso: npm run dev e abra http://localhost:1420/preview.html (ou tools/snapshot.swift).
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { DEFAULT_SETTINGS } from "./App";
import Card from "./components/Card";
import type { Lang } from "./i18n";
import type { Notification } from "./types";
import "./styles.css";
import type { OnboardingStatus, WelcomeApi } from "./welcome/api";
import Welcome from "./welcome/Welcome";
import type { CustomizeApi } from "./customize/api";
import Customize from "./customize/Customize";

const lang = (new URLSearchParams(location.search).get("lang") as Lang) ?? "pt-BR";

const base = { message: null, title: null, project: null, url: null, ttl_ms: 8000, progress: null };
const samples: Notification[] = [
  { ...base, id: 1, source: "ci", status: "info", title: "PR #445 · Corrige o parser de datas", message: "3 de 12 checks concluídos", project: "pondworks/breadcrumbs#445", progress: { done: 3, total: 12, failed: 0 } },
  { ...base, id: 2, source: "ci", status: "fail", title: "PR #446 · Atualiza dependências", message: "7 de 12 checks concluídos · 1 falhou", project: "pondworks/breadcrumbs#446", progress: { done: 7, total: 12, failed: 1 } },
  { ...base, id: 3, source: "ci", status: "ok", title: "PR #447 · Ajusta o README", message: "12 checks passaram", project: "pondworks/breadcrumbs#447", progress: { done: 12, total: 12, failed: 0 } },
  { ...base, id: 6, source: "ci", status: "ok", sticky: true, title: "PR #445 · Platform: AI Assistant panel with streaming responses and tool calls", message: "All 10 checks passed", project: "pondworks/breadcrumbs-studio#445", progress: { done: 10, total: 10, failed: 0 } },
  { ...base, id: 4, source: "agent", status: "ok", title: "Claude Code", message: "“adicione testes para o parser de datas”", project: "miau-watch" },
  { ...base, id: 5, source: "agent", status: "attention", title: "Claude Code", message: "Claude needs your permission to use Bash", project: "miau-watch" },
];

const params = new URLSearchParams(location.search);

// ?view=welcome: boas-vindas com um estado de exemplo (padrão: Claude ok, gh deslogado).
const mockStatus: OnboardingStatus = {
  claude_connected: params.get("claude") !== "no",
  cursor_present: true,
  cursor_connected: false,
  github: (params.get("github") as OnboardingStatus["github"] | null) ?? "gh_logged_out",
  github_device_flow: params.get("device") === "1",
  nowplaying: false,
};
const mockApi: WelcomeApi = {
  settings: async () => ({ ...DEFAULT_SETTINGS, lang }),
  status: async () => mockStatus,
  connectClaude: async () => "ok",
  enableGithub: async () => {},
  test: async () => {},
  finish: async () => {},
  deviceStart: async () => ({ user_code: "WDJB-MJHT", verification_uri: "https://github.com/login/device" }),
  deviceWait: () => new Promise<string>(() => {}),
};

const root = createRoot(document.getElementById("root")!);
// ?view=customize: Personalizar com o laço ligado (cor por ?color=%23rrggbb).
const customizeApi: CustomizeApi = {
  settings: async () => ({
    ...DEFAULT_SETTINGS,
    lang,
    accessory_enabled: params.get("bow") !== "0",
    accessory_color: params.get("color") ?? "#ff6996",
  }),
  save: async () => ({ ...DEFAULT_SETTINGS, lang }),
};

if (params.get("view") === "customize") {
  root.render(
    <StrictMode>
      <Customize api={customizeApi} />
    </StrictMode>,
  );
} else if (params.get("view") === "welcome") {
  root.render(
    <StrictMode>
      <Welcome api={mockApi} />
    </StrictMode>,
  );
} else root.render(
  <StrictMode>
    <div className="preview-grid">
      {samples.map((n) => (
        <div className="preview-slot" key={n.id}>
          <Card
            notification={n}
            lang={lang}
            leaving={false}
            hovered={new URLSearchParams(location.search).get("hover") === String(n.id)}
            pixelSize={3}
            onActivate={() => {}}
            onDismiss={() => {}}
          />
        </div>
      ))}
    </div>
  </StrictMode>,
);
