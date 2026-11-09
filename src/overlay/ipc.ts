import { invoke } from "@tauri-apps/api/core";

function quiet(promise: Promise<unknown>) {
  promise.catch((e) => console.warn(e));
}

export const overlayIpc = {
  setSize: (width: number, height: number) => quiet(invoke("set_overlay_size", { width, height })),
  setInteractive: (interactive: boolean) => quiet(invoke("set_interactive", { interactive })),
  cardAction: (url: string | null) => quiet(invoke("card_action", { url })),
  empty: (lastSeenId: number) => quiet(invoke("overlay_empty", { lastSeenId })),
  alertCleared: () => quiet(invoke("tray_alert_cleared")),
};
