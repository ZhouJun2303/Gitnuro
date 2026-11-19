export type Shortcut =
  | "refresh"
  | "commit"
  | "exit"
  | "up"
  | "down"
  | "pull"
  | "push"
  | "branch"
  | "stash"
  | "open"
  | "newTab"
  | "closeTab"
  | "tabLeft"
  | "tabRight"
  | "settings"
  | "switchLanguage"
  | "toggleLite"
  | "launch"
  | "back"
  | "forward"
  | "fetch"
  | "quickFetch"
  | "quickPull"
  | "quickPush"
  | "tag"
  | "clone"
  | "init"
  | "changes"
  | "commits"
  | "reveal"
  | "zoomIn"
  | "zoomOut"
  | "search"
  | "commitPush"
  | "stageToggle"
  | "stageAll"
  | "discard"
  | "explorer"
  | "terminal"
  | "filterBranch"
  | "reopen"
  | "pastePatch"
  | "selectAll"
  | "performance"
  | "copy";

type Chord = { key: string; ctrl?: boolean; shift?: boolean; alt?: boolean; meta?: boolean };

const mac =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(`${navigator.platform} ${navigator.userAgent}`);

const commandOnMac = new Set<Shortcut>([
  "commit",
  "refresh",
  "pull",
  "push",
  "branch",
  "stash",
  "open",
  "newTab",
  "closeTab",
  "settings",
  "switchLanguage",
  "toggleLite",
  "launch",
  "fetch",
  "quickFetch",
  "quickPull",
  "quickPush",
  "tag",
  "clone",
  "init",
  "changes",
  "commits",
  "reveal",
  "zoomIn",
  "zoomOut",
  "search",
  "commitPush",
  "stageToggle",
  "stageAll",
  "discard",
  "explorer",
  "terminal",
  "filterBranch",
  "reopen",
  "pastePatch",
  "selectAll",
  "performance",
  "copy",
]);

const base: Array<[Shortcut, Chord[]]> = [
  ["refresh", [{ key: "F5" }, { key: "r", ctrl: true }]],
  ["commit", [{ key: "Enter", ctrl: true }]],
  ["exit", [{ key: "Escape" }]],
  ["up", [{ key: "ArrowUp" }]],
  ["down", [{ key: "ArrowDown" }]],
  ["pull", [{ key: "l", ctrl: true, shift: true }]],
  ["push", [{ key: "p", ctrl: true, shift: true }]],
  ["branch", [{ key: "b", ctrl: true, shift: true }]],
  ["stash", [{ key: "h", ctrl: true, shift: true }]],
  ["open", [{ key: "o", ctrl: true }]],
  ["newTab", [{ key: "t", ctrl: true }]],
  ["closeTab", [{ key: "w", ctrl: true }]],
  ["tabLeft", [{ key: "Tab", ctrl: true, shift: true }]],
  ["tabRight", [{ key: "Tab", ctrl: true }]],
  ["settings", [{ key: ",", ctrl: true }]],
  ["switchLanguage", [{ key: "l", ctrl: true, alt: true }]],
  ["toggleLite", [{ key: "m", ctrl: true, alt: true, shift: true }]],
  ["launch", [{ key: "k", ctrl: true }, { key: "p", ctrl: true }]],
  ["back", [{ key: "ArrowLeft", alt: true }]],
  ["forward", [{ key: "ArrowRight", alt: true }]],
  ["fetch", [{ key: "f", ctrl: true, shift: true }]],
  ["quickFetch", [{ key: "f", ctrl: true, alt: true, shift: true }]],
  ["quickPull", [{ key: "l", ctrl: true, alt: true, shift: true }]],
  ["quickPush", [{ key: "p", ctrl: true, alt: true, shift: true }]],
  ["tag", [{ key: "t", ctrl: true, alt: true }]],
  ["clone", [{ key: "n", ctrl: true }]],
  ["init", [{ key: "n", ctrl: true, shift: true }]],
  ["changes", [{ key: "1", ctrl: true }]],
  ["commits", [{ key: "2", ctrl: true }]],
  ["reveal", [{ key: "0", ctrl: true }]],
  ["zoomIn", [{ key: "=", ctrl: true }]],
  ["zoomOut", [{ key: "-", ctrl: true }]],
  ["search", [{ key: "f", ctrl: true }]],
  ["commitPush", [{ key: "Enter", ctrl: true, shift: true }]],
  ["stageToggle", [{ key: "s", ctrl: true, shift: true }]],
  ["stageAll", [{ key: "s", ctrl: true, alt: true, shift: true }]],
  ["discard", [{ key: "Delete" }, { key: "d", ctrl: true, shift: true }, { key: "Backspace" }]],
  ["explorer", [{ key: "o", ctrl: true, alt: true }]],
  ["terminal", [{ key: "t", ctrl: true, alt: true, shift: true }]],
  ["filterBranch", [{ key: "a", ctrl: true, shift: true }]],
  ["selectAll", [{ key: "a", ctrl: true }]],
  ["reopen", [{ key: "t", ctrl: true, shift: true }]],
  ["pastePatch", [{ key: "v", ctrl: true, shift: true }]],
  ["performance", [{ key: "p", ctrl: true, alt: true }]],
  ["copy", [{ key: "c", ctrl: true }]],
];

function chords(action: Shortcut, items: Chord[]): Chord[] {
  if (!mac || !commandOnMac.has(action)) return items;
  return items.map((item) => (item.ctrl ? { ...item, ctrl: false, meta: true } : item));
}

const defaults = base.map(([action, items]) => ({ action, items: chords(action, items) }));
let bindings = defaults;

function label(chord: Chord) {
  const parts: string[] = [];
  if (chord.meta) parts.push("⌘");
  if (chord.ctrl) parts.push("Ctrl");
  if (chord.alt) parts.push("Alt");
  if (chord.shift) parts.push("Shift");
  parts.push(chord.key.length === 1 ? chord.key.toUpperCase() : chord.key);
  return parts.join("+");
}

function parseChord(value: string): Chord | null {
  if (typeof value !== "string") return null;
  const parts = value.split("+");
  const rawKey = parts.pop() ?? "";
  const key = rawKey.length === 1 ? rawKey.toLowerCase() : rawKey;
  if (!key || !(key.length === 1 || /^(F(?:[1-9]|1\d|2[0-4])|Escape|Enter|Tab|Delete|Backspace|Arrow(?:Up|Down|Left|Right))$/.test(key))) return null;
  const modifiers = new Set(parts);
  if (modifiers.size !== parts.length || parts.some((part) => !["Ctrl", "⌘", "Alt", "Shift"].includes(part))) return null;
  return { key, ctrl: modifiers.has("Ctrl"), meta: modifiers.has("⌘"), alt: modifiers.has("Alt"), shift: modifiers.has("Shift") };
}

export function configureShortcuts(overrides: Record<string, string[]> = {}) {
  const claimed = new Set<string>();
  bindings = defaults.map(({ action, items }) => {
    const custom = overrides[action];
    const chosen = Array.isArray(custom) ? custom.map(parseChord).filter((item): item is Chord => item !== null) : items;
    return { action, items: chosen.filter((item) => {
      const key = label(item);
      if (claimed.has(key)) return false;
      claimed.add(key);
      return true;
    }) };
  });
}

export function defaultShortcutLabels(action: Shortcut) {
  return defaults.find((item) => item.action === action)?.items.map(label) ?? [];
}

export function shortcutLabels(action: Shortcut, overrides?: Record<string, string[]>) {
  const custom = overrides?.[action];
  if (Array.isArray(custom)) return custom.map(parseChord).filter((item): item is Chord => item !== null).map(label);
  return bindings.find((item) => item.action === action)?.items.map(label) ?? [];
}

export function shortcutOwner(value: string, exclude: Shortcut): Shortcut | null {
  return bindings.find((binding) => binding.action !== exclude && binding.items.some((item) => label(item) === value))?.action ?? null;
}

export function capturedShortcut(event: KeyboardEvent): string | null {
  if (event.isComposing || ["Control", "Meta", "Alt", "Shift", "Dead", "Process"].includes(event.key)) return null;
  const key = event.key === "+" ? "=" : event.key === "_" ? "-" : event.key.length === 1 ? event.key.toLowerCase() : event.key;
  const chord = parseChord(`${event.metaKey ? "⌘+" : ""}${event.ctrlKey ? "Ctrl+" : ""}${event.altKey ? "Alt+" : ""}${event.shiftKey ? "Shift+" : ""}${key}`);
  return chord ? label(chord) : null;
}

function sameKey(event: KeyboardEvent, key: string) {
  if (event.key === key) return true;
  if (key === "=" && (event.key === "+" || event.code === "Equal" || event.code === "NumpadAdd")) return true;
  if (key === "-" && (event.key === "_" || event.code === "Minus" || event.code === "NumpadSubtract")) return true;
  return event.key.length === 1 && event.key.toLowerCase() === key;
}

export function shortcut(event: KeyboardEvent): Shortcut | null {
  for (const binding of bindings) {
    for (const chord of binding.items) {
      if (!sameKey(event, chord.key)) continue;
      if (event.ctrlKey !== !!chord.ctrl) continue;
      if (event.metaKey !== !!chord.meta) continue;
      if (event.shiftKey !== !!chord.shift) continue;
      if (event.altKey !== !!chord.alt) continue;
      return binding.action;
    }
  }
  return null;
}

export function shortcutLabel(action: Shortcut) {
  return shortcutLabels(action)[0] ?? "";
}

export function typing(event: KeyboardEvent) {
  const tag = (event.target as HTMLElement | null)?.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || (event.target as HTMLElement | null)?.isContentEditable === true;
}
