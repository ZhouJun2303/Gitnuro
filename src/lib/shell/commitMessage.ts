import type { CommitTypeRecord } from "../status";

/** Built-in types, also the defaults in `commit-template.txt`. */
export const COMMIT_TYPE_DEFAULTS: readonly CommitTypeRecord[] = [
  { name: "feat", color: "#1a7f37", label: "" },
  { name: "fix", color: "#cf222e", label: "" },
  { name: "docs", color: "#0969da", label: "" },
  { name: "art", color: "#bf3989", label: "" },
  { name: "style", color: "#8250df", label: "" },
  { name: "refactor", color: "#9a6700", label: "" },
  { name: "test", color: "#0f766e", label: "" },
  { name: "chore", color: "#52525b", label: "" },
  { name: "perf", color: "#c2410c", label: "" },
  { name: "revert", color: "#4338ca", label: "" },
];

export const COMMIT_TYPE_COLORS: Record<string, string> = Object.fromEntries(
  COMMIT_TYPE_DEFAULTS.map((item) => [item.name, item.color]),
);

const FALLBACK_COLOR = "#1a7f37";
const COLOR = /^#[0-9a-f]{6}$/i;
const NAME = /^[a-z0-9][a-z0-9_-]*$/;

export type CommitType = string;

export function isBuiltinCommitType(name: string) {
  return COMMIT_TYPE_DEFAULTS.some((item) => item.name === name);
}

/** Trim and drop unusable rows. An empty list means the built-in defaults. */
export function commitTypesOf(records: readonly CommitTypeRecord[] | undefined): CommitTypeRecord[] {
  const source = records?.length ? records : COMMIT_TYPE_DEFAULTS;
  const out: CommitTypeRecord[] = [];
  for (const record of source) {
    const name = record.name.trim().toLowerCase();
    if (!NAME.test(name) || out.some((item) => item.name === name)) continue;
    const color = record.color?.trim() ?? "";
    out.push({
      name,
      color: COLOR.test(color) ? color.toLowerCase() : (COMMIT_TYPE_COLORS[name] ?? FALLBACK_COLOR),
      label: record.label?.trim() ?? "",
    });
  }
  return out.length ? out : COMMIT_TYPE_DEFAULTS.map((item) => ({ ...item }));
}

/** A name the composer can put before the colon. Lowercase keeps `splitSubject` matching. */
export function commitTypeName(value: string) {
  return value.trim().toLowerCase();
}

export function validCommitTypeName(name: string) {
  return NAME.test(name);
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

let registry: CommitTypeRecord[] = commitTypesOf(undefined);
let registryNames = new Set(registry.map((item) => item.name));
let SUBJECT = subjectPattern(registry);
let LEADING_TYPE = leadingPattern(registry);

function subjectPattern(types: readonly CommitTypeRecord[]) {
  return new RegExp(`^(?:${types.map((item) => escapeRegExp(item.name)).join("|")}):[^\\S\\r\\n]*\\S.*$`);
}

function leadingPattern(types: readonly CommitTypeRecord[]) {
  return new RegExp(`^(${types.map((item) => escapeRegExp(item.name)).join("|")})[^\\S\\r\\n]*[:：][^\\S\\r\\n]*`, "i");
}

/** Point the helpers below at the configured types. Called when settings load or change. */
export function setCommitTypes(records: readonly CommitTypeRecord[] | undefined) {
  registry = commitTypesOf(records);
  registryNames = new Set(registry.map((item) => item.name));
  SUBJECT = subjectPattern(registry);
  LEADING_TYPE = leadingPattern(registry);
}

/** The configured types, in display order. */
export function commitTypes() {
  return registry;
}

export function isCommitType(value: string): value is CommitType {
  return registryNames.has(value);
}

export function commitTypeColor(type: string) {
  return registry.find((item) => item.name === type)?.color ?? FALLBACK_COLOR;
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
    if (!match || !isCommitType(match[2].toLowerCase())) return [];
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
      if (!match || !isCommitType(match[2].toLowerCase())) return line;
      return `${match[1]}${match[2].padEnd(width, " ")} ${match[3].trim()}`;
    })
    .join("\n");
  return { subject, description };
}
