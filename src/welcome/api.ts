import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../types";

/** Espelho de `onboarding::Status` no Rust. */
export interface OnboardingStatus {
  claude_connected: boolean;
  cursor_present: boolean;
  cursor_connected: boolean;
  github: "on" | "ready" | "gh_logged_out" | "no_login";
  github_device_flow: boolean;
  nowplaying: boolean;
}

/** De onde a tela de boas-vindas lê e para onde manda as ações (trocável nos testes). */
export interface WelcomeApi {
  settings(): Promise<Settings>;
  status(): Promise<OnboardingStatus>;
  /** "ok" ou "manual" (não deu para editar o arquivo; o trecho foi copiado). */
  connectClaude(): Promise<string>;
  enableGithub(): Promise<void>;
  test(): Promise<void>;
  finish(): Promise<void>;
}

export const tauriWelcomeApi: WelcomeApi = {
  settings: () => invoke<Settings>("get_settings"),
  status: () => invoke<OnboardingStatus>("onboarding_status"),
  connectClaude: () => invoke<string>("onboarding_connect_claude"),
  enableGithub: () => invoke("onboarding_enable_github"),
  test: () => invoke("onboarding_test"),
  finish: () => invoke("onboarding_finish"),
};
