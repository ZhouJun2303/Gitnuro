/** A changed path returned by the Rust `workspace_status` command. */
export type ChangeKind = "added" | "modified" | "deleted" | "renamed" | "conflict" | "untracked";

export type StatusFile = {
  path: string;
  kind: ChangeKind;
  previousPath: string | null;
};

export type StatusSnapshot = {
  path: string;
  branch: string | null;
  staged: StatusFile[];
  unstaged: StatusFile[];
};

export type DiffLineKind = "add" | "delete" | "hunk" | "context" | "meta";

export type DiffLine = {
  kind: DiffLineKind;
  text: string;
};

export type FileDiff = {
  path: string;
  staged: boolean;
  binary: boolean;
  truncated: boolean;
  lines: DiffLine[];
};

export type BadgeTone = "added" | "modified" | "deleted";

/** Single-letter badge used in the file list. */
export function badge(kind: ChangeKind): { letter: string; tone: BadgeTone } {
  switch (kind) {
    case "added":
      return { letter: "A", tone: "added" };
    case "modified":
      return { letter: "M", tone: "modified" };
    case "deleted":
      return { letter: "D", tone: "deleted" };
    case "renamed":
      return { letter: "R", tone: "modified" };
    case "conflict":
      return { letter: "C", tone: "deleted" };
    case "untracked":
      return { letter: "?", tone: "added" };
  }
}
