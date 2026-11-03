import { invoke } from "../perf";
import { commits, diffs, sampleImages, sampleNotices, samplePulls, sampleReflog, staged as sampleStaged, unstaged as sampleUnstaged, type FileChange } from "../sample";
import { highlight } from "../highlight";
import { branchGroup, changedRanges, commitGraph, forkPath, formatWhen, gravatarUrl, laneColors, markTokens, numberedDiff, splitDiff, windowSlice, type GraphCell, type SplitRow } from "../view";
import {
    badge,
    type BlameLine,
    type CommandRecord,
    type CommitDetail,
    type CommitRow,
    type ConflictSides,
    type ConflictView,
    type DiffLine,
    type FileDiff,
    type FilePreview,
    type InProgress,
    type RefSnapshot,
    type Settings,
    type StatusFile,
    type StatusSnapshot,
    type Suggestion,
    type UpdateNotice,
  } from "../status";
import type { Mode, Side, Row, Dialog, MergePiece, RepoFacts, MenuItem, ConfirmAsk, TreeEntry, RepoEntry, CommandEntry, RepoJob, PrefTab, LaunchItem, Place, SearchKey } from "./types";
import type { ShellState } from "./state.svelte";

export function sampleRow(shell: ShellState, file: FileChange): Row {
    return {
      path: file.path,
      letter: file.kind,
      tone: file.kind === "A" ? "added" : file.kind === "D" || file.kind === "C" ? "deleted" : "modified",
    };
  }

export function setConflictsOnly(shell: ShellState, on: boolean) {
    shell.conflictsOnly = on;
    if (!on) return;
    const visible = [...shell.shownUnstaged, ...shell.shownStaged];
    if (shell.selectedPath && visible.some((file) => file.path === shell.selectedPath)) return;
    const next = shell.shownUnstaged[0] ?? shell.shownStaged[0];
    if (!next) return;
    shell.selectFile(next.path, shell.shownUnstaged[0] ? "unstaged" : "staged");
  }

export function discardableEntries(shell: ShellState, indexes: number[]) {
    if (shell.selectedSide !== "unstaged") return [];
    return indexes
      .map((index) => ({ line: shell.diffLines[index], index }))
      .filter((entry): entry is { line: DiffLine; index: number } => !!entry.line && entry.line.workAt != null && (entry.line.kind === "add" || entry.line.kind === "delete"));
  }

function wideCode(code: number) {
  return (code >= 0x1100 && code <= 0x115f)
    || (code >= 0x2e80 && code <= 0xa4cf)
    || (code >= 0xac00 && code <= 0xd7a3)
    || (code >= 0xf900 && code <= 0xfaff)
    || (code >= 0xfe30 && code <= 0xfe4f)
    || (code >= 0xff00 && code <= 0xff60)
    || (code >= 0xffe0 && code <= 0xffe6);
}

export function textColumns(shell: ShellState, text: string) {
    let wide = false;
    for (let index = 0; index < text.length; index++) {
      const code = text.charCodeAt(index);
      if (code === 9 || code >= 0x1100) {
        wide = true;
        break;
      }
    }
    if (!wide) return text.length;
    let column = 0;
    for (let index = 0; index < text.length;) {
      const code = text.codePointAt(index) ?? 0;
      if (code === 9 && !shell.settings.showWhitespace) column = (Math.floor(column / 8) + 1) * 8;
      else column += wideCode(code) ? 2 : 1;
      index += code > 0xffff ? 2 : 1;
    }
    return column;
  }

export function sampleSplitOf(shell: ShellState, lines: string[]) {
    return splitDiff(
      lines.map((text) => ({
        kind: text.startsWith("@@") ? "hunk" as const : text.startsWith("+") && !text.startsWith("+++") ? "add" as const : text.startsWith("-") && !text.startsWith("---") ? "delete" as const : "context" as const,
        text,
      })),
    );
  }

export function pickSplitSide(shell: ShellState, event: MouseEvent) {
    const side = (event.target as HTMLElement).closest("[data-side]")?.getAttribute("data-side");
    if (side === "left" || side === "right") shell.splitPick = side;
  }

export function splitColumnsOf(shell: ShellState, rows: SplitRow[]) {
    let left = 0;
    let right = 0;
    for (const row of rows) {
      left = Math.max(left, textColumns(shell, row.left));
      right = Math.max(right, textColumns(shell, row.right));
    }
    const mark = shell.settings.showDiffMarks ? 1 : 0;
    return { left: left + mark, right: right + mark };
  }

export function cachedSplit(shell: ShellState, file: string, lines: DiffLine[]) {
    const key = `${shell.selectedCommit ?? ""}:${file}:${shell.settings.showDiffMarks ? 1 : 0}`;
    const hit = shell.patchSplits.get(key);
    if (hit && hit.lines === lines) return hit;
    if (shell.patchSplits.size > 40) shell.patchSplits.clear();
    const rows = splitDiff(lines);
    const value = { lines, rows, columns: splitColumnsOf(shell, rows) };
    shell.patchSplits.set(key, value);
    return value;
  }

export function splitScroller(shell: ShellState, node: HTMLElement, columns?: { left: number; right: number }) {
    const sides = ["left", "right"] as const;
    const bar = (side: string) => node.querySelector<HTMLElement>(`.split-bar[data-bar="${side}"]`);
    let wide = columns;
    const apply = () => {
      for (const side of sides) {
        const track = bar(side);
        if (!track?.firstElementChild) continue;
        const chars = wide?.[side] ?? 0;
        (track.firstElementChild as HTMLElement).style.width = chars > 0 ? `calc(${chars}ch + 16px)` : "";
        node.style.setProperty(`--sx-${side}`, `${track.scrollLeft}px`);
      }
    };
    const onScroll = (event: Event) => {
      const target = event.target as HTMLElement;
      const side = target.dataset?.bar;
      if (side) node.style.setProperty(`--sx-${side}`, `${target.scrollLeft}px`);
    };
    const onWheel = (event: WheelEvent) => {
      const delta = event.shiftKey && !event.deltaX ? event.deltaY : event.deltaX;
      if (!delta || Math.abs(delta) < Math.abs(event.shiftKey ? 0 : event.deltaY)) return;
      const hit = (event.target as HTMLElement).closest<HTMLElement>("[data-side], [data-bar]");
      const side = hit?.dataset.side ?? hit?.dataset.bar ?? shell.splitPick;
      const track = bar(side);
      if (!track) return;
      event.preventDefault();
      track.scrollLeft += delta;
    };
    node.addEventListener("scroll", onScroll, true);
    node.addEventListener("wheel", onWheel, { passive: false });
    apply();
    return {
      update(next?: { left: number; right: number }) {
        wide = next;
        apply();
      },
      destroy() {
        node.removeEventListener("scroll", onScroll, true);
        node.removeEventListener("wheel", onWheel);
      },
    };
  }

export function mergeToolLabel(shell: ShellState) {
    const named = shell.settings.mergeToolName.trim();
    if (named) return named;
    const command = shell.settings.mergeTool.trim();
    if (!command) return "";
    const program = command.split(/\s+/)[0] ?? "";
    const base = program.split(/[\\/]/).pop() ?? program;
    return base.replace(/\.exe$/i, "");
}

export function describeToolError(shell: ShellState, kind: "merge" | "diff", error: unknown) {
    const name = (kind === "merge" ? shell.settings.mergeToolName : shell.settings.diffToolName).trim();
    const path = (kind === "merge" ? shell.settings.mergeToolPath : shell.settings.diffToolPath).trim();
    return [shell.tr("chrome.toolFailed", { name: name || shell.tr("chrome.toolUnnamed") }), shell.tr("chrome.toolPathLine", { path: path || shell.tr("chrome.toolPathEmpty") }), "", shell.message(error)].join("\n");
}

export async function launchExternal(shell: ShellState, kind: "merge" | "diff", file: string) {
    if (!file || shell.busy) return;
    try {
      await invoke("launch_tool", { path: shell.repoPath(), kind, file });
    } catch (error) {
      shell.actionError = describeToolError(shell, kind, error);
    }
}

export async function openConflictTool(shell: ShellState) {
    const file = shell.selectedPath;
    if (!file || !mergeToolLabel(shell) || shell.busy) return;
    await launchExternal(shell, "merge", file);
}

export async function applyConflictChoice(shell: ShellState, ours: boolean, theirs: boolean) {
    const file = shell.selectedPath;
    const view = shell.activeConflict;
    if (!file || !view) return;
    if (shell.mode === "live" && shell.busy) return;
    if (ours && theirs) {
      await shell.openTextMerge(file);
      return;
    }
    if (!ours && !theirs || shell.mode !== "live") return;
    await shell.mutate({ action: "resolve", file, side: ours ? "ours" : "theirs", text: "" });
  }

export async function loadDiff(shell: ShellState) {
    if (shell.mode !== "live" || !shell.selectedPath || shell.section !== "changes") {
      shell.diff = null;
      shell.diffError = null;
      shell.conflictView = null;
      return;
    }
    const token = ++shell.diffToken;
    const file = shell.selectedPath;
    const stagedSide = shell.selectedSide === "staged";
    const path = shell.repoPath();
    const rows = stagedSide ? shell.snapshot?.staged : shell.snapshot?.unstaged;
    const conflicted = rows?.some((item) => item.path === file && item.kind === "conflict") ?? false;
    await shell.withWork(shell.tr("chrome.readingFile"), async () => {
    try {
      if (conflicted) {
        const view = await invoke<ConflictView>("conflict_view", { path, file });
        if (token === shell.diffToken && shell.viewing(path) && shell.selectedPath === file) {
          shell.conflictView = view;
          shell.diff = null;
          shell.diffError = null;
          shell.preview = null;
          shell.imageNew = null;
          shell.imageOld = null;
        }
        return;
      }
      shell.conflictView = null;
      const value = await invoke<FileDiff>("file_diff", {
        path,
        file,
        staged: stagedSide,
        unified: shell.settings.showEntireFile ? 100000 : (shell.settings.diffContext ?? 3),
        ignoreSpace: shell.settings.ignoreSpace,
      });
      if (token === shell.diffToken && shell.viewing(path)) {
        if (!shell.same(shell.diff, value)) shell.diff = value;
        shell.diffError = null;
        if (value.binary) {
          shell.preview = await invoke<FilePreview | null>("file_preview", { path, file });
          const older = await invoke<FilePreview | null>("revision_preview", { path, rev: "HEAD", file }).catch(() => null);
          shell.imageNew = shell.preview?.dataUrl ?? null;
          shell.imageOld = older?.dataUrl ?? null;
        } else {
          shell.preview = null;
          shell.imageNew = null;
          shell.imageOld = null;
        }
      }
    } catch (error) {
      if (token === shell.diffToken && shell.viewing(path)) {
        shell.diff = null;
        shell.conflictView = null;
        shell.diffError = shell.message(error);
      }
    }
    });
  }

export function tabText(shell: ShellState, path: string) {
    const named = shell.settings.tabLabels.find((item) => item.startsWith(`${path}\t`));
    return named ? named.slice(path.length + 1) : shell.folderName(path);
  }

export function tabColor(shell: ShellState, path: string) {
    const named = shell.settings.tabColors.find((item) => item.startsWith(`${path}\t`));
    return named ? named.slice(path.length + 1) : "";
  }

export function pairSetting(shell: ShellState, list: string[], path: string, value: string) {
    return [...list.filter((item) => !item.startsWith(`${path}\t`)), ...(value.trim() ? [`${path}\t${value.trim()}`] : [])];
  }

export function setLayout(shell: ShellState, next: "list" | "tree" | "combined") {
    shell.settings = { ...shell.settings, fileLayout: next, treeFiles: next === "tree" };
    void shell.savePrefs(false);
  }

export async function toggleFakeIgnore(shell: ShellState) {
    shell.settings = { ...shell.settings, fakeIgnore: shell.settings.fakeIgnore === false };
    await shell.savePrefs(false);
    if (shell.mode !== "live") return;
    try {
      shell.applySnapshot(await invoke<StatusSnapshot>("workspace_status", { path: shell.repoPath() }));
      await shell.loadDiff();
    } catch (error) {
      shell.actionError = shell.message(error);
    }
  }

export function openFileView(shell: ShellState, event: MouseEvent) {
    event.stopPropagation();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    shell.submenu = null;
    shell.menu = { x: rect.left, y: rect.bottom + 4, items: shell.fileViewMenu() };
  }

export function moveHunk(shell: ShellState, delta: number) {
    const nodes: number[] = [];
    shell.diffLines.forEach((line, index) => {
      if (line.kind === "hunk") nodes.push(index);
    });
    if (nodes.length === 0) return;
    shell.hunkAt = (shell.hunkAt + delta + nodes.length) % nodes.length;
    const index = nodes[shell.hunkAt] ?? 0;
    shell.diffAnchor = index;
    shell.diffSelected = [index];
    if (shell.settings.diffStyle === "split") {
      shell.diffEl?.querySelector(`[data-line="${index}"]`)?.scrollIntoView({ block: "center" });
      return;
    }
    const top = Math.max(0, index * 18 - 36);
    shell.diffTop = top;
    if (shell.diffEl) shell.diffEl.scrollTop = top;
  }

export function copyPatch(shell: ShellState, indexes?: number[]) {
    const lines = shell.diff?.lines ?? [];
    const chosen = new Set(indexes && indexes.length > 0 ? indexes : lines.map((_, index) => index));
    if (indexes && indexes.length > 0) {
      for (const index of indexes) {
        for (let cursor = index; cursor >= 0; cursor -= 1) {
          const kind = lines[cursor]?.kind;
          if (kind === "hunk") {
            chosen.add(cursor);
            break;
          }
          if (kind === "meta") break;
        }
      }
    }
    const text = [...chosen]
      .sort((left, right) => left - right)
      .map((index) => lines[index]?.text ?? "")
      .filter((line) => line.length > 0)
      .join("\n");
    if (text) shell.copyText(text);
  }

export function groupLabel(shell: ShellState, folder: string) {
    const named = shell.settings.groupNames.find((item) => item.startsWith(`${folder}\t`));
    return named ? named.slice(folder.length + 1) : shell.folderName(folder);
  }

export function fitImage(shell: ShellState, event: Event) {
    const image = event.currentTarget as HTMLImageElement;
    const side = Math.max(image.naturalWidth, image.naturalHeight);
    const scale = side > 0 && side < 128 ? Math.floor(128 / side) : 1;
    image.style.width = scale > 1 ? `${image.naturalWidth * scale}px` : "";
    image.style.imageRendering = scale > 1 ? "pixelated" : "";
  }

export async function paintPixel(shell: ShellState, left: string, right: string) {
    const load = (src: string) =>
      new Promise<HTMLImageElement>((resolve, reject) => {
        const image = new Image();
        image.onload = () => resolve(image);
        image.onerror = () => reject(new Error("image"));
        image.src = src;
      });
    try {
      const [before, after] = await Promise.all([load(left), load(right)]);
      const width = Math.max(before.width, after.width);
      const height = Math.max(before.height, after.height);
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      const context = canvas.getContext("2d");
      if (!context) return;
      context.drawImage(before, 0, 0);
      const leftData = context.getImageData(0, 0, width, height);
      context.clearRect(0, 0, width, height);
      context.drawImage(after, 0, 0);
      const rightData = context.getImageData(0, 0, width, height);
      const out = context.createImageData(width, height);
      for (let index = 0; index < out.data.length; index += 4) {
        out.data[index] = Math.abs(leftData.data[index] - rightData.data[index]);
        out.data[index + 1] = Math.abs(leftData.data[index + 1] - rightData.data[index + 1]);
        out.data[index + 2] = Math.abs(leftData.data[index + 2] - rightData.data[index + 2]);
        out.data[index + 3] = 255;
      }
      context.putImageData(out, 0, 0);
      shell.pixelUrl = canvas.toDataURL();
    } catch {
      shell.pixelUrl = "";
    }
  }

export function selectChangeFile(shell: ShellState, path: string) {
    if (!path) return;
    shell.historyFile = path;
    shell.commitCollapsed = shell.expandParents(path, shell.commitCollapsed);
    shell.revealSelectedRow();
    if (shell.mode === "live") void shell.pickCommitFile(path);
  }

export function selectTreeFile(shell: ShellState, path: string) {
    if (!path) return;
    shell.historyFile = path;
    shell.treeCollapsed = shell.expandParents(path, shell.treeCollapsed);
    shell.blobTop = 0;
    shell.preview = null;
    shell.treeBinary = false;
    if (shell.mode === "live") void shell.openTreeFile(path);
    shell.revealSelectedRow();
  }

export async function showBlame(shell: ShellState, file: string) {
    shell.blameLines = null;
    await shell.withWork(shell.tr("chrome.readingFile"), async () => {
      try {
        shell.blameLines = await invoke<BlameLine[]>("blame_file", { path: shell.repoPath(), file });
      } catch (error) {
        shell.actionError = shell.message(error);
      }
    });
  }

export function flatEntries(shell: ShellState, files: Row[]) {
    return [...files]
      .sort((a, b) => a.path.localeCompare(b.path))
      .map((file) => ({ key: `file:${file.path}`, kind: "file" as const, path: file.path, file, depth: 0 }));
  }

export function asTree(shell: ShellState, files: Row[], collapsed = shell.collapsedDirs) {
    type Node = { name: string; path: string; kind: "dir" | "file"; file?: Row; children: Node[] };
    const root: Node = { name: "", path: "", kind: "dir", children: [] };
    for (const file of files) {
      const parts = file.path.split(/[/\\]/);
      let node = root;
      let acc = "";
      parts.forEach((part, index) => {
        acc = acc ? `${acc}/${part}` : part;
        const last = index === parts.length - 1;
        let child = node.children.find((item) => item.name === part);
        if (!child) {
          child = { name: part, path: acc, kind: last ? "file" : "dir", file: last ? file : undefined, children: [] };
          node.children.push(child);
        } else if (last) {
          child.kind = "file";
          child.file = file;
        }
        node = child;
      });
    }
    const sortNodes = (nodes: Node[]) => {
      nodes.sort((a, b) => (a.kind === b.kind ? a.name.localeCompare(b.name) : a.kind === "dir" ? -1 : 1));
      for (const node of nodes) sortNodes(node.children);
    };
    sortNodes(root.children);
    const hidden = new Set(collapsed);
    const flat: TreeEntry[] = [];
    const walk = (nodes: Node[], depth: number) => {
      for (const node of nodes) {
        flat.push({ key: `${node.kind}:${node.path}`, kind: node.kind, path: node.path, file: node.file, depth });
        if (node.kind === "dir" && !hidden.has(node.path)) walk(node.children, depth + 1);
      }
    };
    walk(root.children, 0);
    return flat;
  }

export function toggleDir(shell: ShellState, path: string) {
    shell.collapsedDirs = shell.collapsedDirs.includes(path) ? shell.collapsedDirs.filter((item) => item !== path) : [...shell.collapsedDirs, path];
  }

export function toggleCommitDir(shell: ShellState, path: string) {
    shell.commitCollapsed = shell.commitCollapsed.includes(path) ? shell.commitCollapsed.filter((item) => item !== path) : [...shell.commitCollapsed, path];
  }

export function paint(shell: ShellState, text: string, path: string) {
    const tokens = shell.settings.disableSyntaxHighlight || shell.heavyWrap ? [{ text, cls: "" }] : highlight(text, path);
    if (!shell.settings.showWhitespace) return tokens;
    return tokens.map((token) => ({ ...token, text: token.text.replace(/ /g, "·").replace(/\t/g, "→") }));
  }

export function sampleBlob(shell: ShellState, path: string) {
    const lines = diffs[path];
    if (!lines) return "";
    return lines
      .filter((line) => !line.startsWith("@@") && !line.startsWith("-"))
      .map((line) => (line.startsWith("+") || line.startsWith(" ") ? line.slice(1) : line))
      .join("\n");
  }

export function fileKind(shell: ShellState, path: string) {
    const base = path.split(/[/\\]/).pop() ?? "";
    const dot = base.lastIndexOf(".");
    const ext = dot >= 0 ? base.slice(dot + 1).toLowerCase() : "";
    if (["png", "jpg", "jpeg", "gif", "webp", "ico", "bmp", "avif"].includes(ext)) return "image";
    if (ext === "json") return "json";
    if (["xml", "html", "htm", "svg", "xaml", "plist", "prefab", "csproj", "uxml"].includes(ext)) return "markup";
    if (["md", "txt", "rst", "adoc"].includes(ext)) return "text";
    if (["cs", "kt", "kts", "java", "ts", "tsx", "js", "jsx", "mjs", "cjs", "rs", "py", "go", "c", "cc", "cpp", "h", "hpp", "svelte", "vue", "css", "scss", "less", "php", "rb", "swift", "lua", "dart", "scala", "sh", "ps1", "sql"].includes(ext)) return "code";
    return "file";
  }

export function expandParents(shell: ShellState, path: string, collapsed: string[]) {
    const parts = path.split(/[/\\]/);
    const open = new Set<string>();
    let acc = "";
    for (let index = 0; index < parts.length - 1; index += 1) {
      acc = acc ? `${acc}/${parts[index]}` : (parts[index] ?? "");
      open.add(acc);
    }
    return collapsed.filter((item) => !open.has(item));
  }

export function collapseTreeExcept(shell: ShellState, paths: string[], focus: string | null) {
    const dirs = new Set<string>();
    for (const path of paths) {
      const parts = path.split(/[/\\]/);
      let acc = "";
      for (let index = 0; index < parts.length - 1; index += 1) {
        acc = acc ? `${acc}/${parts[index]}` : (parts[index] ?? "");
        dirs.add(acc);
      }
    }
    const open = new Set(focus ? dirs : []);
    if (focus) {
      open.clear();
      const parts = focus.split(/[/\\]/);
      let acc = "";
      for (let index = 0; index < parts.length - 1; index += 1) {
        acc = acc ? `${acc}/${parts[index]}` : (parts[index] ?? "");
        open.add(acc);
      }
    }
    return [...dirs].filter((dir) => !open.has(dir));
  }

export function markedLine(shell: ShellState, text: string, other: string, path: string) {
    const tokens = shell.paint(text, path);
    const plain = tokens.map((token) => ({ ...token, hot: false }));
    if (!other || other === text) return plain;
    const width = tokens.reduce((sum, token) => sum + token.text.length, 0);
    if (width !== text.length) return plain;
    return markTokens(tokens, changedRanges(other, text));
  }

export function patchOf(shell: ShellState, file: string) {
    return shell.patchCache[file] ?? (shell.historyDiff?.path === file ? shell.historyDiff : null);
  }

export function pickedIn(shell: ShellState, rows: Row[]) {
    const names = new Set(rows.map((file) => file.path));
    return shell.picked.filter((path) => names.has(path));
  }

export async function discardPicked(shell: ShellState, files = [...shell.picked]) {
    for (const file of files) await shell.mutate({ action: "discard", file });
    shell.picked = shell.picked.filter((path) => !files.includes(path));
  }

export async function unstagePicked(shell: ShellState, files = [...shell.picked]) {
    if (files.length === 0) return;
    await shell.mutate({ action: "stagePaths", files, unstage: true });
    shell.picked = shell.picked.filter((path) => !files.includes(path));
  }

export async function stagePicked(shell: ShellState, files = [...shell.picked]) {
    if (files.length === 0) return;
    await shell.mutate({ action: "stagePaths", files, unstage: false });
    shell.picked = shell.picked.filter((path) => !files.includes(path));
  }

export function stageChosen(shell: ShellState, side: Side, files: string[]) {
    if (files.length > 1) {
      if (side === "staged") void shell.unstagePicked(files);
      else void shell.stagePicked(files);
      return;
    }
    const file = files[0];
    if (file) shell.runChange(side === "staged" ? "unstage_path" : "stage_path", { file });
  }

function rowOnSide(shell: ShellState, path: string, side: Side) {
    const rows = side === "staged" ? shell.staged : shell.unstaged;
    return rows.find((item) => item.path === path);
  }

function readingOnly(shell: ShellState) {
    const job = shell.currentJob;
    // A double-click selects the file twice, so more than one diff read can be in flight.
    // Writes refuse to start while the repository is busy, so this label is still only a read.
    return !shell.switching && job?.label === shell.tr("chrome.readingFile");
  }

const flipping = new Set<string>();

/** Open or unclosed conflict markers. A cleaned file is safe to stage. */
function conflictMarkers(working: string) {
    let open = false;
    for (const line of working.split("\n")) {
      if (line.startsWith("<<<<<<<")) {
        open = true;
        continue;
      }
      if (open && line.startsWith(">>>>>>>")) return true;
    }
    return open;
  }

async function conflictCanStage(shell: ShellState, repo: string, path: string) {
    try {
      const view = await invoke<ConflictView>("conflict_view", { path: repo, file: path });
      if (shell.repoPath() !== repo) return false;
      if (view.binary) {
        shell.actionError = shell.tr("chrome.stageConflictBinary");
        return false;
      }
      const sides = await invoke<ConflictSides>("conflict_sides", { path: repo, file: path });
      if (shell.repoPath() !== repo || shell.dialog) return false;
      if (conflictMarkers(sides.working)) {
        shell.actionError = null;
        await shell.openTextMerge(path);
        shell.mergeLeft = view.theirs.label || shell.mergeLeft;
        shell.mergeRight = view.ours.label || shell.mergeRight;
        return false;
      }
      return true;
    } catch (error) {
      if (shell.repoPath() !== repo) return false;
      const text = shell.message(error);
      // Status can still say "conflict" after the index was already resolved.
      if (text.includes("is not unmerged")) return true;
      shell.actionError = text;
      return false;
    }
  }

/** Double-click moves one file to the other side. Conflicts are not staged blindly. */
export async function flipStage(shell: ShellState, path: string, side: Side) {
    if (shell.mode !== "live" || !path || shell.dialog || shell.menu || shell.launchOpen) return;
    const repo = shell.repoPath();
    const file = rowOnSide(shell, path, side);
    if (!repo || !file) return;
    const key = `${shell.jobKey(repo)}\0${side}\0${path}`;
    if (flipping.has(key)) return;
    if (shell.busy && !readingOnly(shell)) {
      shell.actionError = shell.tr("chrome.stageBusy");
      return;
    }
    flipping.add(key);
    try {
      if (file.letter === "C") {
        const ready = await conflictCanStage(shell, repo, path);
        if (!ready || shell.repoPath() !== repo || shell.dialog || shell.menu || shell.launchOpen) return;
        if (!rowOnSide(shell, path, side)) return;
      }
      const reading = readingOnly(shell);
      if (shell.busy && !reading) {
        shell.actionError = shell.tr("chrome.stageBusy");
        return;
      }
      const changed = await shell.runChange(side === "staged" ? "unstage_path" : "stage_path", { file: path }, reading);
      if (changed === undefined && shell.mode === "live" && shell.repoPath() === repo) {
        shell.actionError = shell.tr("chrome.stageBusy");
      }
    } finally {
      flipping.delete(key);
    }
  }

export async function deletePicked(shell: ShellState, files = [...shell.picked]) {
    for (const file of files) await shell.mutate({ action: "delete", file });
    shell.picked = shell.picked.filter((path) => !files.includes(path));
  }

export function confirmPicked(shell: ShellState, files: string[], title: string, run: (files: string[]) => Promise<void>) {
    if (files.length === 0 || shell.mode !== "live") return;
    shell.ask(title, files.join("\n"), () => void run(files));
  }

export function discardChosen(shell: ShellState, files: string[]) {
    const names = files.filter((path) => path.length > 0);
    if (names.length === 0 || shell.mode !== "live") return;
    if (names.length === 1) {
      shell.ask(shell.tr("chrome.discardSelected"), shell.tr("dialog.discardBody", { name: names[0] ?? "" }), () => void shell.mutate({ action: "discard", file: names[0] }));
      return;
    }
    shell.ask(shell.tr("chrome.discardSelected"), `${shell.tr("dialog.discardFilesBody", { count: String(names.length) })}\n${names.join("\n")}`, () => void shell.discardPicked(names));
  }

export function deleteChosen(shell: ShellState, files: string[]) {
    const names = files.filter((path) => path.length > 0);
    if (names.length === 0 || shell.mode !== "live") return;
    if (names.length === 1) {
      shell.ask(shell.tr("menu.deleteFile"), shell.tr("dialog.deleteBody", { name: names[0] ?? "" }), () => void shell.mutate({ action: "delete", file: names[0] }));
      return;
    }
    shell.confirmPicked(names, shell.tr("chrome.deleteSelected"), shell.deletePicked.bind(shell));
  }

export function openStash(shell: ShellState, files: string[]) {
    const names = [...new Set(files.filter((path) => path.length > 0))];
    shell.draft = "";
    const rows = [...shell.unstaged, ...shell.staged];
    const selectedUntracked = names.some((path) => rows.some((file) => file.path === path && (file.letter === "A" || file.letter === "?")));
    shell.includeUntracked = shell.settings.stashIncludeUntracked || selectedUntracked;
    shell.stashFiles = names;
    shell.dialog = "stash";
  }

export function ignoreTargets(shell: ShellState, path: string, side: Side) {
    const rows = side === "staged" ? shell.shownStaged : shell.shownUnstaged;
    const selected = shell.pickedIn(rows);
    if (selected.length > 1 && selected.includes(path)) return selected;
    return [path];
  }

export async function addIgnore(shell: ShellState, paths: string[], fake: boolean, directory: boolean) {
    const files = [...new Set(paths.filter((path) => path.length > 0))];
    if (files.length === 0 || shell.mode !== "live" || shell.busy) return;
    if (fake && shell.settings.fakeIgnore === false) {
      shell.settings = { ...shell.settings, fakeIgnore: true };
      await shell.savePrefs(false);
    }
    await shell.mutate({ action: "appendIgnore", files, fake, directory });
    shell.picked = shell.picked.filter((path) => !files.includes(path));
  }

export function filesInDir(shell: ShellState, dir: string, side: Side) {
    const source = side === "staged" ? shell.shownStaged : shell.shownUnstaged;
    return source
      .filter((file) => file.path === dir || file.path.startsWith(`${dir}/`) || file.path.startsWith(`${dir}\\`))
      .map((file) => file.path);
  }

export function stageFolder(shell: ShellState, dir: string, unstage: boolean) {
    const files = shell.filesInDir(dir, unstage ? "staged" : "unstaged");
    if (files.length > 0) void shell.mutate({ action: "stagePaths", files, unstage });
  }

export function copyText(shell: ShellState, text: string) {
    const fallback = () => {
      const area = document.createElement("textarea");
      area.value = text;
      area.setAttribute("readonly", "");
      area.style.position = "fixed";
      area.style.opacity = "0";
      area.style.left = "0";
      area.style.top = "0";
      document.body.appendChild(area);
      area.focus({ preventScroll: true });
      area.select();
      document.execCommand("copy");
      area.remove();
    };
    const clipboard = navigator.clipboard;
    if (!clipboard?.writeText) {
      fallback();
      return;
    }
    void clipboard.writeText(text).catch(() => fallback());
  }

export function clearTextSelection(shell: ShellState) {
    const had = Boolean(shell.heldText || shell.textWhere || shell.textFile);
    shell.heldText = null;
    shell.textWhere = null;
    shell.textFile = null;
    if (!had) return;
    document.querySelectorAll<HTMLElement>(".select-all").forEach((node) => {
      node.classList.remove("select-all");
      node.removeAttribute("data-select-side");
    });
    window.getSelection()?.removeAllRanges();
  }

export function rememberPointer(shell: ShellState, event: MouseEvent) {
    if (event.button !== 0) return;
    shell.clearTextSelection();
    const node = event.target instanceof Element ? event.target : null;
    if (!node || node.closest("input, textarea, select, [contenteditable='true'], .palette, .dialog, .menu")) return;
    const blob = node.closest<HTMLElement>(".blob-body");
    if (blob) {
      shell.keyTarget = "blob";
      shell.selectRoot = blob;
      return;
    }
    const card = node.closest<HTMLElement>(".fork-card");
    if (card) {
      shell.keyTarget = "detail";
      shell.selectRoot = card;
      return;
    }
    const diffBody = node.closest<HTMLElement>(".diff-body");
    if (diffBody || node.closest("section.diff, .detail-pane")) {
      shell.keyTarget = "diff";
      shell.selectRoot = diffBody;
      return;
    }
    if (node.closest(".staged-pane")) {
      shell.keyTarget = "staged";
      shell.selectRoot = null;
      return;
    }
    if (node.closest(".unstaged-pane")) {
      shell.keyTarget = shell.fileLayout === "combined" ? "combined" : "unstaged";
      shell.selectRoot = null;
      return;
    }
    if (node.closest(".commit-row")) {
      shell.keyTarget = "commits";
      shell.selectRoot = null;
    }
  }

export function diffSide(shell: ShellState, root: HTMLElement) {
    if (!root.classList.contains("split")) return "all";
    if (root.classList.contains("pick-right")) return "right";
    if (root.classList.contains("pick-left")) return "left";
    return shell.splitPick;
  }

export function blockText(shell: ShellState, root: HTMLElement, side: "left" | "right" | "all") {
    if ((side === "left" || side === "right") && root === shell.diffEl && shell.splitRows.length > 0) {
      const mark = side === "left" ? "delete" : "add";
      return shell.splitRows
        .map((row) => {
          const kind = side === "left" ? row.leftKind : row.rightKind;
          const text = side === "left" ? row.left : row.right;
          return (shell.settings.showDiffMarks && kind === mark ? (side === "left" ? "-" : "+") : "") + text;
        })
        .join("\n");
    }
    if (side === "left" || side === "right") {
      return [...root.querySelectorAll<HTMLElement>(`.split-code[data-side="${side}"] .split-text`)]
        .map((node) => node.textContent ?? "")
        .join("\n");
    }
    if (root.classList.contains("blob-body")) return shell.blobLines.join("\n");
    if (root.classList.contains("fork-card")) {
      const subject = root.querySelector(".detail-message")?.textContent?.trim() ?? "";
      const body = root.querySelector(".detail-body-text")?.textContent ?? "";
      return [subject, body].filter((part) => part.length > 0).join("\n\n");
    }
    if (root.querySelector("button.virtual-row") && shell.blameLines) return shell.blameLines.map((line) => line.text).join("\n");
    if (shell.section === "changes" && root.closest("section.diff")) {
      return shell.mode === "live" ? (shell.diff?.lines ?? []).map((line) => line.text).join("\n") : shell.selectedDiff.join("\n");
    }
    if (root.tagName === "PRE") return root.textContent ?? "";
    const lines = [...root.querySelectorAll<HTMLElement>(":scope > span")];
    if (lines.length > 0) return lines.map((node) => node.textContent ?? "").join("\n");
    return root.innerText || "";
  }

export function activeTextRoot(shell: ShellState) {
    if (shell.selectRoot?.isConnected) return shell.selectRoot;
    if (shell.keyTarget === "blob") return document.querySelector<HTMLElement>(".blob-body");
    if (shell.keyTarget === "detail") return document.querySelector<HTMLElement>(".fork-card");
    if (shell.keyTarget === "diff") return document.querySelector<HTMLElement>(shell.section === "changes" ? "section.diff .diff-body" : ".detail-pane .diff-body, .detail .diff-body");
    return null;
  }

export function fileTargetRows(shell: ShellState) {
    let target = shell.keyTarget;
    if (target !== "staged" && target !== "unstaged" && target !== "combined") target = shell.selectedSide === "staged" && shell.shownStaged.length > 0 ? "staged" : "unstaged";
    if (target === "staged" && shell.shownStaged.length === 0 && shell.shownUnstaged.length > 0) target = shell.fileLayout === "combined" ? "combined" : "unstaged";
    if (target === "unstaged" && shell.shownUnstaged.length === 0 && shell.shownStaged.length > 0) target = "staged";
    const rows = target === "staged" ? shell.shownStaged : target === "combined" ? [...shell.shownStaged, ...shell.shownUnstaged] : shell.shownUnstaged;
    return { target, rows };
  }

export function selectAllFiles(shell: ShellState) {
    shell.clearTextSelection();
    const { target, rows } = shell.fileTargetRows();
    if (rows.length === 0) return;
    shell.keyTarget = target;
    shell.picked = [...new Set(rows.map((file) => file.path))];
    const side: Side = target === "staged" ? "staged" : "unstaged";
    const keep = rows.some((file) => file.path === shell.selectedPath && (target === "combined" || shell.selectedSide === side));
    if (target === "staged" || target === "unstaged") shell.selectedSide = side;
    if (!keep) {
      const first = rows[0];
      if (!first) return;
      shell.selectedPath = first.path;
      if (target === "staged") shell.selectedSide = "staged";
      else if (target === "unstaged") shell.selectedSide = "unstaged";
      else shell.selectedSide = shell.shownStaged.some((file) => file.path === first.path) ? "staged" : "unstaged";
      shell.blameLines = null;
      if (shell.mode === "live") void shell.loadDiff();
    }
  }

export function collapseFileSelection(shell: ShellState, rows: Row[]) {
    const keep = rows.find((file) => file.path === shell.selectedPath) ?? rows[0];
    if (!keep) {
      shell.picked = [];
      return;
    }
    shell.picked = [keep.path];
    if (shell.selectedPath === keep.path) return;
    shell.selectedPath = keep.path;
    if (shell.keyTarget === "staged") shell.selectedSide = "staged";
    else if (shell.keyTarget === "unstaged") shell.selectedSide = "unstaged";
    shell.blameLines = null;
    if (shell.mode === "live") void shell.loadDiff();
  }

export function toggleAllDiffLines(shell: ShellState) {
    if (shell.blameLines || shell.section !== "changes" || shell.diffLines.length === 0) return false;
    const complete = shell.diffSelected.length === shell.diffLines.length && shell.diffLines.every((_, index) => shell.diffSelected.includes(index));
    shell.clearTextSelection();
    if (complete) {
      shell.diffSelected = [];
      return true;
    }
    shell.diffAnchor = 0;
    shell.diffSelected = shell.diffLines.map((_, index) => index);
    return true;
  }

export function selectAllHere(shell: ShellState) {
    const textZone = shell.keyTarget === "diff" || shell.keyTarget === "blob" || shell.keyTarget === "detail";
    if (textZone && shell.keyTarget === "diff" && shell.toggleAllDiffLines()) {
      if (shell.diffSelected.length > 0) {
        const root = shell.activeTextRoot();
        if (root) {
          const side = shell.diffSide(root);
          const text = shell.blockText(root, side).replace(/\n$/, "");
          if (text.length > 0) {
            shell.heldText = text;
            shell.textSide = side;
          }
        }
      }
      return;
    }
    if (textZone && (shell.heldText || shell.textWhere || shell.textFile)) {
      shell.clearTextSelection();
      return;
    }
    const root = shell.activeTextRoot();
    if (root && textZone) {
      const side = shell.diffSide(root);
      const text = shell.blockText(root, side).replace(/\n$/, "");
      if (text.length > 0) {
        shell.heldText = text;
        shell.textSide = side;
        shell.textFile = null;
        shell.textWhere = null;
        if (shell.keyTarget === "blob") shell.textWhere = "blob";
        else if (shell.keyTarget === "detail") shell.textWhere = "detail";
        else if (shell.section === "changes" && root.closest("section.diff")) shell.textWhere = "changes";
        else shell.textFile = root.dataset.file || root.closest<HTMLElement>("[data-file]")?.dataset.file || null;
        if (!shell.textWhere && !shell.textFile) {
          root.classList.add("select-all");
          root.dataset.selectSide = side;
        }
        return;
      }
    }
    if (shell.section !== "changes") return;
    const { rows } = shell.fileTargetRows();
    if (rows.length > 1 && rows.every((file) => shell.picked.includes(file.path))) {
      shell.collapseFileSelection(rows);
      return;
    }
    shell.selectAllFiles();
  }

export function pathsForCopy(shell: ShellState) {
    if (shell.section !== "changes") return "";
    const rows = shell.keyTarget === "staged" ? shell.shownStaged : shell.keyTarget === "combined" ? [...shell.shownStaged, ...shell.shownUnstaged] : shell.keyTarget === "unstaged" ? shell.shownUnstaged : [];
    const chosen = shell.pickedIn(rows);
    return chosen.length > 1 ? chosen.join("\n") : "";
  }

export function fullPath(shell: ShellState, file: string) {
    const root = shell.snapshot?.path ?? "";
    const sep = root.includes("\\") ? "\\" : "/";
    const relative = sep === "\\" ? file.replaceAll("/", "\\") : file.replaceAll("\\", "/");
    return root ? `${root.replace(/[\\/]+$/, "")}${sep}${relative}` : relative;
  }

export function selectFile(shell: ShellState, path: string, side: Side, event?: MouseEvent) {
    shell.clearTextSelection();
    shell.conflictView = null;
    if (path !== shell.mergeFile) shell.textMerge = false;
    shell.selectedPath = path;
    shell.selectedSide = side;
    shell.blameLines = null;
    shell.diffAnchor = 0;
    shell.diffSelected = [];
    if (event?.shiftKey || event?.ctrlKey || event?.metaKey) {
      shell.picked = shell.picked.includes(path) ? shell.picked.filter((item) => item !== path) : [...shell.picked, path];
    } else {
      shell.picked = [path];
    }
    if (shell.mode === "live") void shell.loadDiff();
  }

export function toRow(shell: ShellState, file: StatusFile) {
    const mark = badge(file.kind);
    return { path: file.path, letter: mark.letter, tone: mark.tone };
  }

export function fileName(shell: ShellState, path: string) {
    const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return slash >= 0 ? path.slice(slash + 1) : path;
  }

export function parentDir(shell: ShellState, path: string) {
    const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return slash >= 0 ? path.slice(0, slash) : "";
  }

export function folderName(shell: ShellState, path: string) {
    const trimmed = path.replace(/[\\/]+$/, "");
    const slash = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
    return slash >= 0 ? trimmed.slice(slash + 1) : trimmed;
  }

export function normalPath(shell: ShellState, path: string) {
    return path.replace(/[\\/]+$/, "").replaceAll("\\", "/").toLowerCase();
  }

export function lineText(shell: ShellState, line: DiffLine) {
    return line.text.startsWith("+") || line.text.startsWith("-") || line.text.startsWith(" ")
      ? line.text.slice(1)
      : line.text;
  }

export function splitLineAt(shell: ShellState, row: SplitRow, side: "left" | "right") {
    if (row.lineIndex != null) return row.lineIndex;
    if (side === "left") return row.leftIndex ?? row.rightIndex ?? null;
    return row.rightIndex ?? row.leftIndex ?? null;
  }

export function selectDiffRange(shell: ShellState, from: number, to: number) {
    const start = Math.min(from, to);
    const end = Math.max(from, to);
    const next: number[] = [];
    for (let index = start; index <= end; index += 1) next.push(index);
    shell.diffSelected = next;
  }

export function toggleDiffLine(shell: ShellState, index: number) {
    shell.diffAnchor = index;
    shell.diffSelected = shell.diffSelected.includes(index) ? shell.diffSelected.filter((item) => item !== index) : [...shell.diffSelected, index].sort((left, right) => left - right);
  }

export function clickDiffLine(shell: ShellState, index: number, event: MouseEvent) {
    const target = event.target instanceof Element ? event.target : null;
    if (target?.closest(".gutter, .split-no")) return;
    const selection = window.getSelection();
    if (selection && !selection.isCollapsed && selection.anchorNode && target?.contains(selection.anchorNode)) return;
    if (event.shiftKey) shell.selectDiffRange(shell.diffAnchor, index);
    else if (event.ctrlKey || event.metaKey) shell.toggleDiffLine(index);
    else {
      shell.diffAnchor = index;
      shell.diffSelected = [index];
    }
  }

export function dragDiffLine(shell: ShellState, index: number, event: MouseEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    if (event.ctrlKey || event.metaKey) {
      shell.toggleDiffLine(index);
      return;
    }
    const anchor = event.shiftKey ? shell.diffAnchor : index;
    if (!event.shiftKey) shell.diffAnchor = index;
    shell.selectDiffRange(anchor, index);
    const move = (ev: MouseEvent) => {
      const body = shell.diffEl;
      if (body) {
        const rect = body.getBoundingClientRect();
        if (ev.clientY < rect.top + 28) body.scrollTop -= 18;
        else if (ev.clientY > rect.bottom - 28) body.scrollTop += 18;
      }
      const hit = document.elementFromPoint(ev.clientX, ev.clientY)?.closest<HTMLElement>("[data-line]");
      const next = Number(hit?.dataset.line);
      if (Number.isInteger(next)) shell.selectDiffRange(anchor, next);
    };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  }

export function chosenDiffLines(shell: ShellState, clicked: number) {
    const source = shell.diffSelected.includes(clicked) ? shell.diffSelected : [clicked];
    let indexes = source.filter((index) => index >= 0 && index < shell.diffLines.length);
    if (indexes.length === 0 && clicked >= 0 && clicked < shell.diffLines.length) indexes = [clicked];
    indexes = [...indexes].sort((left, right) => left - right);
    const same = indexes.length === shell.diffSelected.length && indexes.every((index, at) => shell.diffSelected[at] === index);
    if (!same) {
      shell.diffAnchor = clicked;
      shell.diffSelected = indexes;
    }
    return indexes;
  }

export function hunkOf(shell: ShellState, index: number) {
    let hunk = -1;
    for (let cursor = 0; cursor <= index && cursor < shell.diffLines.length; cursor += 1) {
      if (shell.diffLines[cursor]?.kind === "hunk") hunk += 1;
    }
    return hunk >= 0 ? hunk : null;
  }

export function editorLineNo(shell: ShellState, index: number) {
    const row = shell.shownDiffLines[index];
    const number = Number(row?.newNo || row?.oldNo || 0);
    return number > 0 ? number : 1;
  }

export function removesOnApply(shell: ShellState, line: DiffLine, action: "stageLine" | "discardLine", unstage: boolean) {
    if (action === "discardLine") return line.kind === "add";
    return unstage === (line.kind === "add");
  }

export async function applyLineOps(shell: ShellState, action: "stageLine" | "discardLine", entries: { line: DiffLine; index: number }[]) {
    const file = shell.selectedPath;
    if (!file || entries.length === 0 || !shell.inApp() || shell.busy) return;
    const unstage = shell.selectedSide === "staged";
    const ordered = [...entries]
      .filter((entry) => (action === "stageLine" ? entry.line.stageAt : entry.line.workAt) != null)
      .sort((left, right) => {
        // Positions were computed against the current file. Apply from the bottom,
        // and remove a line before inserting another at the same index.
        const leftAt = (action === "stageLine" ? left.line.stageAt : left.line.workAt) ?? 0;
        const rightAt = (action === "stageLine" ? right.line.stageAt : right.line.workAt) ?? 0;
        if (leftAt !== rightAt) return rightAt - leftAt;
        const leftRemove = shell.removesOnApply(left.line, action, unstage);
        const rightRemove = shell.removesOnApply(right.line, action, unstage);
        if (leftRemove !== rightRemove) return leftRemove ? -1 : 1;
        return right.index - left.index;
      });
    if (ordered.length === 0) return;
    const path = shell.repoPath();
    if (!path) return;
    shell.actionError = null;
    shell.startWork(shell.tr("chrome.working"), path);
    shell.diffSelected = [];
    try {
      let value: (StatusSnapshot & { output?: string }) | null = null;
      for (const entry of ordered) {
        const at = action === "stageLine" ? entry.line.stageAt : entry.line.workAt;
        if (at == null) continue;
        value = await invoke<StatusSnapshot & { output?: string }>("mutate", {
          path,
          request: {
            action,
            file,
            text: shell.lineText(entry.line),
            addition: entry.line.kind === "add",
            at,
            ...(action === "stageLine" ? { unstage } : {}),
          },
        });
      }
      if (shell.viewing(path)) {
        if (value) shell.applySnapshot(value);
        await shell.loadContext();
        await shell.loadDiff();
      }
    } catch (error) {
      if (!shell.viewing(path)) return;
      shell.actionError = shell.message(error);
      if (action === "stageLine" && shell.settings.ignoreSpace) shell.actionError = `${shell.actionError} ${shell.tr("dialog.spaceHint")}`;
      try {
        shell.applySnapshot(await invoke<StatusSnapshot>("workspace_status", { path }));
        await shell.loadContext();
        await shell.loadDiff();
      } catch {
        /* keep the action error */
      }
    } finally {
      shell.stopWork(path);
    }
  }

export async function showLineHistory(shell: ShellState, line: DiffLine | undefined) {
    const text = line ? shell.lineText(line).trim() : "";
    if (!shell.selectedPath || !text) return;
    shell.commandOutput = await invoke<string>("pick_history", { path: shell.repoPath(), file: shell.selectedPath, text }).catch((error) => shell.message(error));
    shell.dialog = "output";
  }
