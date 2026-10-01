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
  | "filterBranch";

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
  ["launch", [{ key: "k", ctrl: true }]],
  ["back", [{ key: "ArrowLeft", alt: true }]],
  ["forward", [{ key: "ArrowRight", alt: true }]],
  ["fetch", [{ key: "f", ctrl: true, shift: true }]],
  ["quickFetch", [{ key: "f", ctrl: true, alt: true, shift: true }]],
  ["quickPull", [{ key: "l", ctrl: true, alt: true, shift: true }]],
  ["quickPush", [{ key: "p", ctrl: true, alt: true, shift: true }]],
  ["tag", [{ key: "t", ctrl: true, shift: true }]],
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
  ["discard", [{ key: "d", ctrl: true, shift: true }, { key: "Backspace" }]],
  ["explorer", [{ key: "o", ctrl: true, alt: true }]],
  ["terminal", [{ key: "t", ctrl: true, alt: true }]],
  ["filterBranch", [{ key: "a", ctrl: true, shift: true }]],
];

function chords(action: Shortcut, items: Chord[]): Chord[] {
  if (!mac || !commandOnMac.has(action)) return items;
  return items.map((item) => (item.ctrl ? { ...item, ctrl: false, meta: true } : item));
}

const bindings = base.map(([action, items]) => ({ action, items: chords(action, items) }));

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
  const binding = bindings.find((item) => item.action === action);
  const chord = binding?.items[0];
  if (!chord) return "";
  const parts: string[] = [];
  if (chord.meta) parts.push("⌘");
  if (chord.ctrl) parts.push("Ctrl");
  if (chord.alt) parts.push("Alt");
  if (chord.shift) parts.push("Shift");
  parts.push(chord.key.length === 1 ? chord.key.toUpperCase() : chord.key);
  return parts.join("+");
}

export function typing(event: KeyboardEvent) {
  const tag = (event.target as HTMLElement | null)?.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || (event.target as HTMLElement | null)?.isContentEditable === true;
}
