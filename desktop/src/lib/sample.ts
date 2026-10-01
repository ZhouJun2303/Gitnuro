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
};

export const unstaged: FileChange[] = [
  { path: "app/src/main/kotlin/com/zhoujun/awegit/ui/status/StatusPane.kt", kind: "M" },
  { path: "app/src/main/kotlin/com/zhoujun/awegit/theme/Theme.kt", kind: "M" },
  { path: "README.md", kind: "A" },
];

export const commits: CommitRow[] = [
  {
    id: "e4c91a2",
    summary: "fix: keep the commit composer visible when the file list is empty",
    author: "Jun",
    when: "2 hours ago",
    badges: ["main"],
  },
  {
    id: "b18de07",
    summary: "feat: remember which sidebar groups are expanded",
    author: "Jun",
    when: "yesterday",
    badges: ["origin/main"],
  },
  {
    id: "90aa14c",
    summary: "refactor: read theme colors from one palette",
    author: "Jun",
    when: "3 days ago",
    badges: [],
  },
  {
    id: "66f02e1",
    summary: "feat: add a quiet empty state for the diff pane",
    author: "Jun",
    when: "last week",
    badges: ["v2.0.0"],
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
