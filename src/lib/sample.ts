export type FileChange = {
  path: string;
  kind: "M" | "A" | "D";
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

/** Web preview file lists. Paths stay unique so the two panes do not highlight the same row. */
export const unstaged: FileChange[] = files([
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

export const sampleNotices = [{ id: "1", title: "Review requested on the theme palette", url: "https://github.com/ZhouJun2303/AweGit", unread: true }];

export const samplePulls = [{ title: "Draw colored commit lanes", author: "jun", branch: "graph", url: "https://github.com/ZhouJun2303/AweGit" }];
