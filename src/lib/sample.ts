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
};

export const unstaged: FileChange[] = [
  { path: "app/src/main/kotlin/com/zhoujun/awegit/ui/status/StatusPane.kt", kind: "M" },
  { path: "app/src/main/kotlin/com/zhoujun/awegit/theme/Theme.kt", kind: "M" },
  { path: "README.md", kind: "A" },
  { path: "art/mark.png", kind: "M" },
];

export const commits: CommitRow[] = [
  {
    id: "e4c91a2",
    summary: "fix: keep the commit composer visible when the file list is empty",
    author: "Jun",
    when: "2 hours ago",
    badges: ["main"],
    parents: ["b18de07"],
    files: [{ path: "app/src/main/kotlin/com/zhoujun/awegit/ui/status/StatusPane.kt", kind: "M" }],
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
    badges: [],
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

export const diffs: Record<string, string[]> = {
  "app/src/main/kotlin/com/zhoujun/awegit/ui/status/StatusPane.kt": [
    "@@ -16,6 +16,7 @@ fun StatusPane(",
    "     val summary = state.summary",
    "-    Text(stringResource(Res.string.uncommited_changes_text_input_label_message))",
    "+    SummaryField(summary, onSummaryChange)",
    "+    DescriptionField(state.description, onDescriptionChange)",
  ],
  "app/src/main/kotlin/com/zhoujun/awegit/theme/Theme.kt": [
    "@@ -8,3 +8,4 @@ val forkLightTheme = forkBase(",
    "-    primary = Color(0xFF0067C0),",
    "+    primary = Color(0xFF007AFF),",
    "     background = Color(0xFFFFFFFF),",
  ],
  "README.md": [
    "@@ -1,2 +1,3 @@",
    " # AweGit",
    "+A fast Git client for Windows and Mac.",
  ],
};

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
