import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../types";

export interface OnboardingStatus {
  claude_connected: boolean;
  cursor_present: boolean;
  cursor_connected: boolean;
  github: "on" | "ready" | "gh_logged_out" | "no_login";
  github_device_flow: boolean;
  nowplaying: boolean;
}

export interface WelcomeApi {
  settings(): Promise<Settings>;
  status(): Promise<OnboardingStatus>;
  connectClaude(): Promise<string>;
  enableGithub(): Promise<void>;
  test(): Promise<void>;
  finish(): Promise<void>;
  deviceStart(): Promise<DevicePrompt>;
  deviceWait(): Promise<string>;
}

export interface DevicePrompt {
  user_code: string;
  verification_uri: string;
}

export const tauriWelcomeApi: WelcomeApi = {
  settings: () => invoke<Settings>("get_settings"),
  status: () => invoke<OnboardingStatus>("onboarding_status"),
  connectClaude: () => invoke<string>("onboarding_connect_claude"),
  enableGithub: () => invoke("onboarding_enable_github"),
  test: () => invoke("onboarding_test"),
  finish: () => invoke("onboarding_finish"),
  deviceStart: () => invoke<DevicePrompt>("github_device_start"),
  deviceWait: () => invoke<string>("github_device_wait"),
};
