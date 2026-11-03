/** Same types as the built-in commit-msg rule and `commit-template.txt`. */
export const COMMIT_TYPES = ["feat", "fix", "docs", "art", "style", "refactor", "test", "chore", "perf", "revert"] as const;

export type CommitType = (typeof COMMIT_TYPES)[number];

const SUBJECT = /^(feat|fix|docs|art|style|refactor|test|chore|perf|revert):[^\S\r\n]*\S.*$/;
const LEADING_TYPE = /^(feat|fix|docs|art|style|refactor|test|chore|perf|revert)[^\S\r\n]*[:：][^\S\r\n]*/i;

export function isCommitType(value: string): value is CommitType {
  return (COMMIT_TYPES as readonly string[]).includes(value);
}

/**
 * Default swatch for each prefix. A later setting can pass a partial map to
 * `commitTypeColor` without changing the picker.
 */
export const COMMIT_TYPE_COLORS: Record<CommitType, string> = {
  feat: "#1a7f37",
  fix: "#cf222e",
  docs: "#0969da",
  art: "#bf3989",
  style: "#8250df",
  refactor: "#9a6700",
  test: "#0f766e",
  chore: "#52525b",
  perf: "#c2410c",
  revert: "#4338ca",
};

export function commitTypeColor(type: string, overrides?: Partial<Record<string, string>>) {
  const custom = overrides?.[type]?.trim();
  if (custom) return custom;
  return isCommitType(type) ? COMMIT_TYPE_COLORS[type] : COMMIT_TYPE_COLORS.feat;
}

export function typeMessageKey(type: string) {
  return `chrome.type${type.slice(0, 1).toUpperCase()}${type.slice(1)}`;
}

function unwrapLine(text: string) {
  let value = text.replace(/\r\n/g, "\n").split("\n")[0]?.trim() ?? "";
  value = value.replace(/^\uFEFF/, "").replace(/^[-*]\s+/, "").trim();
  const wrapped = /^(['"`])([\s\S]*)\1$/.exec(value);
  if (wrapped) value = wrapped[2].trim();
  value = value.replace(/^\*+|\*+$/g, "").trim();
  return value;
}

/**
 * Pull every leading `type:` off a subject. Repeated prefixes collapse, and the
 * last recognized type wins, so `feat: feat: 新增` becomes type `feat` and `新增`.
 */
export function splitSubject(text: string) {
  let rest = unwrapLine(text);
  let type: CommitType | "" = "";
  let peeled = false;
  for (let i = 0; i < 8; i += 1) {
    const match = LEADING_TYPE.exec(rest);
    if (!match) break;
    const kind = match[1].toLowerCase();
    if (!isCommitType(kind)) break;
    type = kind;
    rest = rest.slice(match[0].length).trim();
    peeled = true;
  }
  return { type, text: rest, peeled };
}

/** Join the dropdown type and the description. A prefix still typed in the text is not repeated. */
export function joinSubject(type: string, text: string) {
  const split = splitSubject(text);
  const kind = (split.peeled ? split.type : "") || (isCommitType(type) ? type : "");
  const body = split.text.trim();
  if (!kind || !body) return "";
  return `${kind}: ${body}`;
}

/** Drop a body line that only repeats the subject the model already returned. */
export function withoutRepeatedSubject(description: string, type: string, text: string) {
  const body = description.replace(/\r\n/g, "\n").trim();
  if (!body) return "";
  const lines = body.split("\n");
  const first = (lines[0] ?? "").trim();
  const split = splitSubject(first);
  const joined = joinSubject(type, text);
  const repeats =
    (joined.length > 0 && first === joined) ||
    (split.peeled && text.trim().length > 0 && split.text.trim() === text.trim() && (!split.type || split.type === type));
  if (!repeats) return body;
  return lines.slice(1).join("\n").replace(/^\n+/, "").trim();
}

export function subjectFitsConvention(summary: string) {
  return SUBJECT.test(summary.trim());
}

/** True when every non-empty line is git commentary, like the type guide. */
export function isCommentary(text: string) {
  const lines = text
    .replace(/\r\n/g, "\n")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  return lines.length > 0 && lines.every((line) => line.startsWith("#"));
}

/**
 * Split a commit template into the subject seed and a body whose type names
 * share one column, so the descriptions line up in a monospace field.
 */
export function alignCommitTemplate(template: string) {
  const lines = template.replace(/\r\n/g, "\n").replace(/\r/g, "\n").split("\n");
  const subject = (lines[0] ?? "").trim();
  const rest = lines.slice(1);
  const details = rest.flatMap((line) => {
    const match = /^(\s*#\s*)([A-Za-z]+)\s+(\S.*)$/.exec(line);
    if (!match || !(COMMIT_TYPES as readonly string[]).includes(match[2])) return [];
    return [match[2]];
  });
  const width = Math.max(8, ...details.map((type) => type.length));
  const description = rest
    .join("\n")
    .replace(/^\n/, "")
    .trim()
    .split("\n")
    .map((line) => {
      const match = /^(\s*#\s*)([A-Za-z]+)\s+(\S.*)$/.exec(line);
      if (!match || !(COMMIT_TYPES as readonly string[]).includes(match[2])) return line;
      return `${match[1]}${match[2].padEnd(width, " ")} ${match[3].trim()}`;
    })
    .join("\n");
  return { subject, description };
}
