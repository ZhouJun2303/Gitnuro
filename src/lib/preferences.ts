import { emit, listen } from "@tauri-apps/api/event";
import { invoke, logClient } from "./perf";
import type { Settings } from "./status";

type Spec = { label: string; shortLabel?: string; options?: readonly (readonly [string, string])[]; min?: number; max?: number; invert?: boolean };
export const preferenceSpecs = {
  diffStyle: { shortLabel: "toolbar.diffStyle", label: "dialog.diffStyle", options: [["split", "dialog.diffSplit"], ["unified", "dialog.diffUnified"]] },
  linesHeight: { shortLabel: "toolbar.linesHeight", label: "dialog.linesHeight", options: [["compact", "dialog.lineCompact"], ["spaced", "dialog.lineSpaced"]] },
  diffFontSize: { shortLabel: "toolbar.diffFontSize", label: "dialog.fontSize", min: 8, max: 32 },
  diffContext: { shortLabel: "toolbar.diffContext", label: "dialog.diffContext", min: 0, max: 100 },
  showEntireFile: { shortLabel: "toolbar.showEntireFile", label: "dialog.wholeFile" },
  diffWordWrap: { shortLabel: "toolbar.diffWordWrap", label: "dialog.wordWrap" },
  showWhitespace: { shortLabel: "toolbar.showWhitespace", label: "dialog.showSpace" },
  ignoreSpace: { shortLabel: "toolbar.ignoreSpace", label: "dialog.ignoreSpace" },
  showDiffMarks: { shortLabel: "toolbar.showDiffMarks", label: "dialog.showMarks" },
  disableSyntaxHighlight: { shortLabel: "toolbar.disableSyntaxHighlight", label: "chrome.syntaxHighlight", invert: true },
  fileLayout: { shortLabel: "toolbar.fileLayout", label: "chrome.fileLayout", options: [["tree", "toolbar.tree"], ["list", "toolbar.list"], ["combined", "toolbar.combined"]] },
  treeFiles: { label: "chrome.treeView" },
  swapPanes: { shortLabel: "toolbar.swapPanes", label: "dialog.swapPanes" },
  fakeIgnore: { label: "chrome.fakeIgnore" },
  commitSort: { label: "dialog.sortCommits", options: [["topo", "dialog.topologically"], ["date", "dialog.byDate"]] },
  maxCommits: { shortLabel: "toolbar.maxCommits", label: "dialog.maxCommits", min: 50, max: 100000 },
  dateRelative: { label: "dialog.dateRelative" },
  date24h: { label: "dialog.date24" },
  compactBranchLabels: { label: "dialog.compactLabels" },
  gravatar: { label: "dialog.gravatar" },
  highlightIssues: { label: "dialog.issueLinks" },
  pushOnCommit: { shortLabel: "toolbar.pushOnCommit", label: "dialog.pushOnCommit" },
  signCommits: { shortLabel: "toolbar.signCommits", label: "dialog.signCommits" },
  spellChecking: { shortLabel: "toolbar.spellChecking", label: "dialog.spellChecking", options: [["disable", "dialog.spellDisable"], ["enable", "dialog.spellEnable"]] },
  aiPromptStyle: { shortLabel: "toolbar.aiPromptStyle", label: "dialog.promptStyle", options: [["short", "chrome.promptShort"], ["long", "chrome.promptLong"]] },
  aiLanguage: { shortLabel: "toolbar.aiLanguage", label: "dialog.aiLanguage", options: [["zh", "dialog.chinese"], ["en", "dialog.english"]] },
  fetchAutomatically: { label: "dialog.fetchAuto" },
  fetchAllRemotes: { label: "dialog.fetchAllRemotes" },
  fetchPrune: { label: "dialog.pruneOnFetch" },
  fetchTags: { label: "dialog.includeTags" },
  pullRebase: { label: "dialog.pullWithRebase" },
  pushAllTags: { label: "dialog.pushAllTags" },
  showWorktrees: { label: "dialog.showWorktrees" },
  verticalTabs: { label: "chrome.verticalTabLayout" },
  tabIndicator: { label: "chrome.tabChanges" },
  flowAutoOpen: { shortLabel: "toolbar.flowAutoOpen", label: "flow.autoOpen" },
  flowAutoClose: { shortLabel: "toolbar.flowAutoClose", label: "flow.autoClose" },
  flowHistoryDays: { shortLabel: "toolbar.flowHistoryDays", label: "flow.historyDays", min: 1, max: 90 },
  errorLogDays: { shortLabel: "toolbar.errorLogDays", label: "dialog.errorLogDays", min: 1, max: 90 },
} as const satisfies Partial<Record<keyof Settings, Spec>>;

export type PreferenceKey = keyof typeof preferenceSpecs;
export type PreferenceValues = Pick<Settings, PreferenceKey>;
export type PreferencePatch = Partial<PreferenceValues>;
export const preferenceKeys = Object.keys(preferenceSpecs) as PreferenceKey[];
export function preferenceSpec(key: PreferenceKey): Spec { return preferenceSpecs[key]; }
export function isPreferenceKey(key: string): key is PreferenceKey { return Object.hasOwn(preferenceSpecs, key); }

export function normalizePreferences(patch: PreferencePatch): PreferencePatch {
  const result: PreferencePatch = {};
  for (const key of preferenceKeys) {
    let value = patch[key];
    if (value === undefined) continue;
    const spec = preferenceSpec(key);
    if (spec.min !== undefined) {
      if (typeof value !== "number" || !Number.isFinite(value)) continue;
      value = Math.max(spec.min, Math.min(spec.max!, Math.round(value)));
    } else if (spec.options) {
      if (!spec.options.some(([option]) => option === value)) continue;
    } else if (typeof value !== "boolean") continue;
    Object.assign(result, { [key]: value });
  }
  if (result.fileLayout !== undefined) result.treeFiles = result.fileLayout === "tree";
  else if (result.treeFiles !== undefined) result.fileLayout = result.treeFiles ? "tree" : "list";
  return result;
}

const eventName = "awegit-preferences-changed";
const desktop = () => "__TAURI_INTERNALS__" in window;

export async function savePreferencePatch(patch: PreferencePatch): Promise<PreferencePatch> {
  const saved = await invoke<Settings>("save_settings", { patch });
  const applied: PreferencePatch = {};
  for (const key of preferenceKeys) if (patch[key] !== undefined) Object.assign(applied, { [key]: saved[key] });
  if (desktop()) {
    await emit(eventName, applied).catch((error) => logClient("warn", "preferences", "broadcast", String(error)));
  } else {
    const channel = new BroadcastChannel(eventName);
    channel.postMessage(applied);
    channel.close();
  }
  return applied;
}

export function onPreferenceChange(update: (patch: PreferencePatch) => void) {
  let active = true;
  let stop = () => {};
  const receive = (value: unknown) => {
    if (active && value && typeof value === "object") update(normalizePreferences(value as PreferencePatch));
  };
  if (desktop()) {
    void listen<PreferencePatch>(eventName, ({ payload }) => receive(payload)).then((unlisten) => {
      if (active) stop = unlisten; else unlisten();
    }).catch((error) => logClient("warn", "preferences", "listen", String(error)));
  } else {
    const channel = new BroadcastChannel(eventName);
    channel.onmessage = ({ data }) => receive(data);
    stop = () => channel.close();
  }
  return () => { active = false; stop(); };
}
