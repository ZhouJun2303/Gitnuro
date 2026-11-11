import { emit, listen } from "@tauri-apps/api/event";
import { logClient } from "./perf";

export type Appearance = "apple" | "light" | "dark";
const eventName = "awegit-appearance-changed";
const desktop = () => "__TAURI_INTERNALS__" in window;
export const isAppearance = (value: unknown): value is Appearance => value === "apple" || value === "light" || value === "dark";

export function resolveAppearance(setting: string, systemDark: boolean): Appearance {
  return isAppearance(setting) ? setting : setting === "system" ? (systemDark ? "dark" : "light") : "apple";
}

export function broadcastAppearance(theme: string) {
  if (!isAppearance(theme) && theme !== "system") return;
  if (desktop()) {
    void emit(eventName, theme).catch((error) => logClient("warn", "appearance", "broadcast", String(error)));
  } else {
    const channel = new BroadcastChannel(eventName);
    channel.postMessage(theme);
    channel.close();
  }
}

export function onAppearanceChange(update: (theme: string) => void) {
  let active = true;
  let stop = () => {};
  const receive = (theme: unknown) => {
    if (active && (isAppearance(theme) || theme === "system")) update(theme);
  };
  if (desktop()) {
    void listen<string>(eventName, ({ payload }) => receive(payload)).then((unlisten) => {
      if (active) stop = unlisten;
      else unlisten();
    }).catch((error) => logClient("warn", "appearance", "listen", String(error)));
  } else {
    const channel = new BroadcastChannel(eventName);
    channel.onmessage = ({ data }) => receive(data);
    stop = () => channel.close();
  }
  return () => { active = false; stop(); };
}
