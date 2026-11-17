/** Read patch headers for the confirmation surface; Git validates the actual patch. */
export function patchFiles(patch: string): string[] {
  const files = new Set<string>();
  for (const line of patch.split(/\r?\n/)) {
    let path = line.startsWith("+++ ") ? line.slice(4).split("\t")[0] : line.startsWith("rename to ") ? line.slice(10) : "";
    if (!path && line.startsWith("diff --git ")) path = line.match(/^diff --git (?:"a\/(?:[^"\\]|\\.)*"|a\/.*?) ("b\/(?:[^"\\]|\\.)*"|b\/.*)$/)?.[1] ?? "";
    if (!path || path === "/dev/null") continue;
    if (path.startsWith('"')) { try { path = JSON.parse(path); } catch { path = path.slice(1, -1); } }
    files.add(path.replace(/^[ab]\//, ""));
  }
  return [...files];
}
