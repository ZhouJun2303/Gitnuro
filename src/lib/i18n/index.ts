import { en } from "./en";
import type { Locale, Messages } from "./types";
import { zh } from "./zh";

const catalogs: Record<Locale, Messages> = { en, zh };

export type { Locale, Messages };
export type MessageKey = string;

export function resolveLocale(setting: string, language = typeof navigator === "undefined" ? "en" : navigator.language): Locale {
  if (setting === "zh" || setting === "en") return setting;
  return language.toLowerCase().startsWith("zh") ? "zh" : "en";
}

export function translate(locale: Locale, key: string, vars: Record<string, string> = {}): string {
  const [group, name] = key.split(".") as [keyof Messages, string];
  const table = catalogs[locale][group] as unknown as Record<string, string> | undefined;
  const fallback = catalogs.en[group] as unknown as Record<string, string> | undefined;
  let text = table?.[name] ?? fallback?.[name] ?? key;
  for (const [token, value] of Object.entries(vars)) text = text.replaceAll(`{${token}}`, value);
  return text;
}
