import { emit, listen } from "@tauri-apps/api/event";
import { logClient } from "../perf";

const eventName = "awegit-locale-changed";
let lastLocale: string | undefined;
const desktop = () => "__TAURI_INTERNALS__" in window;
const valid = (locale: unknown): locale is string => locale === "zh" || locale === "en" || locale === "system";

export function broadcastLocale(locale: string) {
  if (!valid(locale) || locale === lastLocale) return;
  lastLocale = locale;
  if (desktop()) {
    void emit(eventName, locale).catch((error) => logClient("warn", "locale", "broadcast", String(error)));
  } else {
    const channel = new BroadcastChannel(eventName);
    channel.postMessage(locale);
    channel.close();
  }
}

export function onLocaleChange(update: (locale: string) => void) {
  let active = true;
  let stop = () => {};
  const receive = (locale: unknown) => {
    if (!active || !valid(locale)) return;
    lastLocale = locale;
    update(locale);
  };
  if (desktop()) {
    void listen<string>(eventName, ({ payload }) => receive(payload)).then((unlisten) => {
      if (active) stop = unlisten;
      else unlisten();
    }).catch((error) => logClient("warn", "locale", "listen", String(error)));
  } else {
    const channel = new BroadcastChannel(eventName);
    channel.onmessage = ({ data }) => receive(data);
    stop = () => channel.close();
  }
  return () => { active = false; stop(); };
}
