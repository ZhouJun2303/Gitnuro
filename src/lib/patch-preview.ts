const encoder = new TextEncoder(), decoder = new TextDecoder();
const escapes: Record<number, number> = { 97: 7, 98: 8, 102: 12, 110: 10, 114: 13, 116: 9, 118: 11, 34: 34, 92: 92 };

/** Git C-quotes UTF-8 bytes with octal escapes; these are not JSON strings. */
function unquote(path: string): string {
  if (!path.startsWith('"') || !path.endsWith('"')) return path;
  const input = encoder.encode(path.slice(1, -1)), output = new Uint8Array(input.length);
  let count = 0;
  for (let index = 0; index < input.length; index++) {
    const byte = input[index];
    if (byte !== 92 || index + 1 === input.length) { output[count++] = byte; continue; }
    let escaped = input[++index];
    if (escaped >= 48 && escaped <= 55) {
      let value = escaped - 48;
      for (let digits = 1; digits < 3 && input[index + 1] >= 48 && input[index + 1] <= 55; digits++) value = value * 8 + input[++index] - 48;
      output[count++] = value;
    } else {
      if (escapes[escaped] === undefined) output[count++] = 92;
      output[count++] = escapes[escaped] ?? escaped;
    }
  }
  return decoder.decode(output.subarray(0, count));
}

function headerPath(value: string, prefix: string) {
  const path = unquote(value.split("\t", 1)[0]);
  return path.startsWith(prefix) ? path.slice(prefix.length) : path;
}

function diffTarget(header: string) {
  // Without a rename/copy header, both paths are equal. Spaces can include " b/".
  const middle = (header.length - 1) / 2;
  if (Number.isInteger(middle) && header[middle] === " " && header.startsWith("a/") && header.slice(middle + 1).startsWith("b/")
    && header.slice(2, middle) === header.slice(middle + 3)) return header.slice(middle + 3);
  const target = header.match(/^(?:"a\/(?:[^"\\]|\\.)*"|a\/.*?) ("b\/(?:[^"\\]|\\.)*"|b\/.*)$/)?.[1];
  return target ? headerPath(target, "b/") : "";
}

/** Read only file headers; changed content cannot add targets to the confirmation. */
export function patchFiles(patch: string): string[] {
  const files = new Set<string>();
  let old = "", next = "", renamed = "", fallback = "", git = false;
  let oldLines = 0, newLines = 0;
  const flush = () => {
    const path = renamed || (next === "/dev/null" ? old : next) || fallback;
    if (path && path !== "/dev/null") files.add(path);
    old = next = renamed = fallback = "";
  };
  for (let at = 0; at < patch.length;) {
    let end = patch.indexOf("\n", at);
    if (end < 0) end = patch.length;
    const line = patch.slice(at, patch.charCodeAt(end - 1) === 13 ? end - 1 : end);
    at = end + 1;
    if (line.startsWith("diff --git ")) {
      flush(); git = true; oldLines = newLines = 0;
      fallback = diffTarget(line.slice(11));
    } else if (oldLines > 0 || newLines > 0) {
      if (line.startsWith(" ")) { oldLines--; newLines--; }
      else if (line.startsWith("-")) oldLines--;
      else if (line.startsWith("+")) newLines--;
    } else if (line.startsWith("@@ ") || line === "GIT binary patch" || line.startsWith("Binary files ")) {
      if (git) {
        // Skip large bodies without allocating one string for every changed line.
        const marker = patch.indexOf("\ndiff --git ", at - 1);
        if (marker < 0) break;
        at = marker + 1;
      } else {
        const hunk = line.match(/^@@ -\d+(?:,(\d+))? \+\d+(?:,(\d+))? @@/);
        if (hunk) { oldLines = Number(hunk[1] ?? 1); newLines = Number(hunk[2] ?? 1); }
      }
    } else if (line.startsWith("--- ")) {
      if (old) flush();
      old = headerPath(line.slice(4), "a/");
    } else if (line.startsWith("+++ ")) next = headerPath(line.slice(4), "b/");
    else if (line.startsWith("rename to ")) renamed = unquote(line.slice(10));
    else if (line.startsWith("copy to ")) renamed = unquote(line.slice(8));
  }
  flush();
  return [...files];
}
