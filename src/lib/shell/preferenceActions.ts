import { normalizePreferences, preferenceKeys, savePreferencePatch, type PreferencePatch } from "../preferences";
import { rememberDiffReading, restoreDiffReading } from "./diffReading";
import type { ShellState } from "./state.svelte";

type Pending = { tail: Promise<boolean>; revision: number; versions: Map<string, number>; saved: PreferencePatch; error?: string };
const pending = new WeakMap<ShellState, Pending>();
function state(shell: ShellState): Pending {
  let value = pending.get(shell);
  if (!value) {
    value = { tail: Promise.resolve(true), revision: 0, versions: new Map(), saved: {} };
    for (const key of preferenceKeys) Object.assign(value.saved, { [key]: shell.settings[key] });
    pending.set(shell, value);
  }
  return value;
}

export function waitPreferences(shell: ShellState) { return state(shell).tail; }

export function adoptPreferences(shell: ShellState) {
  const current = pending.get(shell);
  if (!current) return;
  for (const key of preferenceKeys) if (!current.versions.has(key)) Object.assign(current.saved, { [key]: shell.settings[key] });
}

export function receivePreferences(shell: ShellState, patch: PreferencePatch) {
  const current = state(shell);
  const available: PreferencePatch = {};
  for (const key of preferenceKeys) {
    if (patch[key] === undefined) continue;
    Object.assign(current.saved, { [key]: patch[key] });
    if (!current.versions.has(key)) Object.assign(available, { [key]: patch[key] });
  }
  applyPreferences(shell, available);
  if (available.errorLogDays !== undefined) shell.pruneErrorLog();
  if (available.fakeIgnore !== undefined) void shell.refresh();
}

export function updatePreferences(shell: ShellState, update: PreferencePatch): Promise<boolean> {
  const patch = normalizePreferences(update);
  if (!Object.keys(patch).length) return Promise.resolve(true);
  const current = state(shell);
  const revision = ++current.revision;
  ++shell.prefsSave; // An older full-settings response must not replace the new controls.
  for (const key of Object.keys(patch)) current.versions.set(key, revision);
  applyPreferences(shell, patch);
  const save = async () => {
    try {
      const saved = shell.backend() ? await savePreferencePatch(patch) : patch;
      const applied: PreferencePatch = {};
      for (const key of preferenceKeys) {
        if (saved[key] === undefined) continue;
        Object.assign(current.saved, { [key]: saved[key] });
        if (current.versions.get(key) !== revision) continue;
        current.versions.delete(key);
        Object.assign(applied, { [key]: saved[key] });
      }
      applyPreferences(shell, applied);
      if (applied.errorLogDays !== undefined) shell.pruneErrorLog();
      if (current.error && shell.actionError === current.error) shell.actionError = null;
      current.error = undefined;
      if (applied.fakeIgnore !== undefined && shell.mode === "live") await shell.refresh();
      return true;
    } catch (error) {
      const rollback: PreferencePatch = {};
      for (const key of preferenceKeys) {
        if (patch[key] === undefined || current.versions.get(key) !== revision) continue;
        current.versions.delete(key);
        Object.assign(rollback, { [key]: current.saved[key] });
      }
      applyPreferences(shell, rollback);
      current.error = `${shell.tr("chrome.preferenceSaveFailed")} ${shell.message(error)}`;
      shell.actionError = current.error;
      return false;
    }
  };
  current.tail = current.tail.then(save, save);
  return current.tail;
}

function applyPreferences(shell: ShellState, patch: PreferencePatch) {
  const changed = preferenceKeys.filter((key) => patch[key] !== undefined && patch[key] !== shell.settings[key]);
  if (!changed.length) return;
  const contentChanged = changed.some((key) => key === "showEntireFile" || key === "diffContext" || key === "ignoreSpace");
  const layoutChanged = changed.some((key) => key === "diffFontSize" || key === "linesHeight" || key === "diffStyle" || key === "diffWordWrap");
  if (contentChanged || layoutChanged) rememberDiffReading(shell);
  shell.settings = { ...shell.settings, ...patch };
  if (layoutChanged) shell.patchHeights = {};
  if (shell.dialog === "fetch") { shell.syncAll = shell.settings.fetchAllRemotes; shell.syncTags = shell.settings.fetchTags; }
  if (shell.dialog === "pull") shell.syncRebase = shell.settings.pullRebase;
  if (shell.dialog === "push") shell.syncTags = shell.settings.pushAllTags;
  if (changed.includes("commitSort") || changed.includes("maxCommits")) {
    shell.logLimit = shell.settings.maxCommits;
    if (shell.mode === "live") void shell.loadContext().then(() => {
      if (shell.selectedCommit) shell.scrollToCommit(shell.selectedCommit);
    }).catch((error) => { shell.actionError = shell.message(error); });
  }
  if (layoutChanged && !contentChanged) restoreDiffReading(shell);
  if (!contentChanged) return;
  shell.diffSelected = [];
  shell.diffPreferencesVersion++;
  shell.clearPatchQueue();
  shell.stalePatchCache = Object.fromEntries(Object.entries({ ...shell.stalePatchCache, ...shell.patchCache }).slice(-100));
  shell.patchCache = {};
  shell.patchVersion++;
  if (shell.mode !== "live") return;
  if (shell.section === "changes" && shell.selectedPath && !shell.selectedConflict) void shell.loadDiff();
  if ((shell.section === "history" || shell.standalone === "history") && shell.historyFile && shell.historyTab !== "tree") {
    void shell.pickCommitFile(shell.historyFile, false);
  }
}
