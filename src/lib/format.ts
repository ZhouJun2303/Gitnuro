const numberFormats = new Map<string, Intl.NumberFormat>();

function numberFormat(locale: string, digits: number) {
  const key = `${locale}:${digits}`;
  let format = numberFormats.get(key);
  if (!format) {
    format = new Intl.NumberFormat(locale === "zh" ? "zh-CN" : "en-US", { maximumFractionDigits: digits });
    numberFormats.set(key, format);
  }
  return format;
}

/** Compact size such as `917 KB` or `1.03 MB`; exact bytes stay available through `formatInt`. */
export function formatBytes(bytes: number, locale = "en") {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${numberFormat(locale, digits).format(value)} ${units[unit]}`;
}

export function formatInt(value: number, locale = "en") {
  return numberFormat(locale, 0).format(value);
}
