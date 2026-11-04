export type FileChange = {
  path: string;
  kind: "M" | "A" | "D" | "C";
};

export type SampleConflictView = {
  ours: { label: string; state: "added" | "modified" | "deleted" | "unchanged"; bytes: number };
  theirs: { label: string; state: "added" | "modified" | "deleted" | "unchanged"; bytes: number };
  binary: boolean;
};

/** Working copies with conflict markers, so sample data can open the text merger. */
export const sampleConflictText: Record<string, { ours: string; theirs: string; working: string }> = {
  "src/merge/both-sides.ts": {
    ours: "export const rate = 2;\n  return rate + 1;",
    theirs: "export const rate = 5;\n  return rate + 4;",
    working: [
      "<<<<<<< main",
      "export const rate = 2;",
      "=======",
      "export const rate = 5;",
      ">>>>>>> feature",
      "export function settle() {",
      "<<<<<<< main",
      "  return rate + 1;",
      "=======",
      "  return rate + 4;",
      ">>>>>>> feature",
      "}",
      "",
    ].join("\n"),
  },
  "src/merge/both-added.ts": {
    ours: "export const created = \"main\";",
    theirs: "export const created = \"feature\";",
    working: ["<<<<<<< main", "export const created = \"main\";", "=======", "export const created = \"feature\";", ">>>>>>> feature", ""].join("\n"),
  },
  "docs/removed-there.md": {
    ours: "# still documented on main",
    theirs: "",
    working: ["<<<<<<< main", "# still documented on main", "=======", ">>>>>>> feature", ""].join("\n"),
  },
};

/** Fixture conflict cards for sample data. Keys match unstaged paths. */
export const sampleConflictViews: Record<string, SampleConflictView> = {
  "data/Y语言表_Data_Language.xlsx": {
    binary: true,
    theirs: { label: "stash/patch", state: "modified", bytes: 56484 },
    ours: { label: "main", state: "modified", bytes: 54061 },
  },
  "src/merge/both-sides.ts": {
    binary: false,
    theirs: { label: "feature", state: "modified", bytes: 48 },
    ours: { label: "main", state: "modified", bytes: 46 },
  },
  "src/merge/both-added.ts": {
    binary: false,
    theirs: { label: "feature", state: "added", bytes: 34 },
    ours: { label: "main", state: "added", bytes: 31 },
  },
  "docs/removed-there.md": {
    binary: false,
    theirs: { label: "feature", state: "deleted", bytes: 0 },
    ours: { label: "main", state: "modified", bytes: 28 },
  },
  "art/removed-here.png": {
    binary: true,
    theirs: { label: "feature", state: "modified", bytes: 18432 },
    ours: { label: "main", state: "deleted", bytes: 0 },
  },
};

export type CommitRow = {
  id: string;
  summary: string;
  author: string;
  when: string;
  badges: string[];
  parents: string[];
  files: FileChange[];
  unpushed?: boolean;
};

const kinds = ["M", "A", "D"] as const;

function files(paths: string[]): FileChange[] {
  return paths.map((path, index) => ({ path, kind: kinds[index % kinds.length] }));
}

/** Sample file lists. Paths stay unique so the two panes do not highlight the same row. */
const sampleUnstagedFiles: FileChange[] = files([
  "app/src/main/kotlin/com/zhoujun/awegit/ui/status/StatusPane.kt",
  "app/src/main/kotlin/com/zhoujun/awegit/theme/Theme.kt",
  "README.md",
  "art/mark.png",
  "src/lib/Shell.svelte",
  "src/lib/view.ts",
  "src/lib/status.ts",
  "src/lib/keys.ts",
  "src/routes/+page.svelte",
  "src-tauri/src/lib.rs",
  "src-tauri/src/commands.rs",
  "crates/git/src/status.rs",
  "crates/git/src/diff.rs",
  "crates/git/src/history.rs",
  "package.json",
  "pnpm-lock.yaml",
  "AGENTS.md",
  "docs/shortcuts.md",
  "scripts/package.mjs",
  "assets/icon.svg",
]);

export const unstaged: FileChange[] = [
  { path: "data/Y语言表_Data_Language.xlsx", kind: "C" },
  sampleUnstagedFiles[0],
  { path: "src/merge/both-sides.ts", kind: "C" },
  sampleUnstagedFiles[1],
  { path: "src/merge/both-added.ts", kind: "C" },
  ...sampleUnstagedFiles.slice(2, 6),
  { path: "docs/removed-there.md", kind: "C" },
  ...sampleUnstagedFiles.slice(6, 10),
  { path: "art/removed-here.png", kind: "C" },
  ...sampleUnstagedFiles.slice(10),
];

export const staged: FileChange[] = files([
  "src/lib/highlight.ts",
  "src/lib/i18n/zh.ts",
  "src/lib/i18n/en.ts",
  "src/lib/clonePath.ts",
  "src/lib/layout.ts",
  "src/app.html",
  "src/app.css",
  "crates/git/src/lib.rs",
  "crates/git/src/write.rs",
  "crates/git/src/refs.rs",
  "crates/git/Cargo.toml",
  "src-tauri/Cargo.toml",
  "src-tauri/tauri.conf.json",
  "vite.config.ts",
  "tsconfig.json",
  "README.zh.md",
  "CHANGELOG.md",
  "LICENSE",
  "art/wordmark.svg",
  "tests/status.test.ts",
]);

export const commits: CommitRow[] = [
  {
    id: "e4c91a2",
    summary: "fix: keep the commit composer visible when the file list is empty",
    author: "Jun",
    when: "2 hours ago",
    badges: ["main"],
    parents: ["b18de07"],
    files: [{ path: "app/src/main/kotlin/com/zhoujun/awegit/ui/status/StatusPane.kt", kind: "M" }],
    unpushed: true,
  },
  {
    id: "b18de07",
    summary: "feat: remember which sidebar groups are expanded",
    author: "Jun",
    when: "yesterday",
    badges: ["origin/main"],
    parents: ["90aa14c"],
    files: [{ path: "README.md", kind: "A" }],
  },
  {
    id: "90aa14c",
    summary: "refactor: read theme colors from one palette",
    author: "Jun",
    when: "3 days ago",
    badges: [],
    parents: ["66f02e1", "c0ffee1"],
    files: [{ path: "app/src/main/kotlin/com/zhoujun/awegit/theme/Theme.kt", kind: "M" }],
  },
  {
    id: "c0ffee1",
    summary: "feat: draw the side branch used by the graph",
    author: "Jun",
    when: "4 days ago",
    badges: ["graph"],
    parents: ["66f02e1"],
    files: [{ path: "art/mark.png", kind: "M" }],
  },
  {
    id: "66f02e1",
    summary: "feat: add a quiet empty state for the diff pane",
    author: "Jun",
    when: "last week",
    badges: ["v2.0.0"],
    parents: [],
    files: [],
  },
];

/** Enough paired edits that the split diff shows at least 400 changed rows. */
function samplePatch(path: string): string[] {
  const name = path.split("/").pop() ?? path;
  const lines: string[] = [];
  const pairs = 42;
  for (let hunk = 0; hunk < 10; hunk += 1) {
    const start = hunk * pairs + 1;
    lines.push(`@@ -${start},${pairs} +${start},${pairs} @@ ${name}`);
    for (let offset = 0; offset < pairs; offset += 1) {
      const line = start + offset;
      lines.push(`-    ${name} previous ${line}`);
      lines.push(`+    ${name} updated ${line}`);
    }
  }
  return lines;
}

export const diffs: Record<string, string[]> = Object.fromEntries(
  [...unstaged, ...staged]
    .filter((file) => !file.path.endsWith(".png"))
    .map((file) => [file.path, samplePatch(file.path)]),
);

/** 8×8 red and 8×8 red with a blue corner, so the preview can switch image modes. */
export const sampleImages = {
  before:
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAGUlEQVR4nGO4Y2PznxLMMGrAqAGjBgwXAwCuWFMfG2uhAwAAAABJRU5ErkJggg==",
  after:
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAIUlEQVR4nGPQiLrzHx++Y2ODFzOMGjAsDCCkYNSAEWEAACEQVZ8m78w/AAAAAElFTkSuQmCC",
};

export const sampleReflog = [
  { id: "e4c91a2", shortId: "e4c91a2", selector: "HEAD@{0}", summary: "commit: keep the commit composer visible" },
  { id: "b18de07", shortId: "b18de07", selector: "HEAD@{1}", summary: "commit: remember sidebar groups" },
];

export const sampleNotices = [{ id: "1", title: "Review requested on the theme palette", url: "https://github.com/ZhouJun2303/Gitnuro", unread: true }];

export const samplePulls = [{ title: "Draw colored commit lanes", author: "jun", branch: "graph", url: "https://github.com/ZhouJun2303/Gitnuro" }];
