/** Separate the shared Git runner's access advice from its original stderr. */
export function fileAccessReport(output: string) {
  if (!output.startsWith("Git operation failed: the following files could not be updated.\n")) return null;
  const lines = output.split("\n");
  const end = lines.findIndex((line) => /^\d+\. After releasing access,/.test(line));
  if (end < 0) return null;
  return { lines: lines.slice(0, end + 1), detail: lines.slice(end + 1).join("\n") };
}
