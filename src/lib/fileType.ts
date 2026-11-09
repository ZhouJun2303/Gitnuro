export type FileMarkKind = "sheet" | "slides" | "doc" | "pdf" | "archive" | "image" | "media" | "binary" | "file";

const kinds: Record<string, FileMarkKind> = {};

function register(kind: FileMarkKind, extensions: string[]) {
  for (const extension of extensions) kinds[extension] = kind;
}

register("sheet", ["xls", "xlsx", "xlsm", "xlsb", "csv", "tsv", "ods", "numbers"]);
register("slides", ["ppt", "pptx", "pptm", "odp", "key"]);
register("doc", ["doc", "docx", "docm", "odt", "rtf", "pages"]);
register("pdf", ["pdf"]);
register("archive", ["zip", "7z", "rar", "tar", "gz", "tgz", "bz2", "xz", "zst", "lz4", "cab"]);
register("image", ["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "svg", "avif", "tif", "tiff", "psd", "tga"]);
register("media", ["mp3", "wav", "flac", "ogg", "m4a", "aac", "wma", "mp4", "mov", "mkv", "webm", "avi", "wmv", "flv", "m4v"]);
register("binary", ["bin", "exe", "dll", "so", "dylib", "class", "jar", "war", "apk", "aab", "ipa", "wasm", "pdb", "lib", "obj", "o", "a", "dmg", "iso", "pak", "unitypackage", "assetbundle"]);

export function fileExtension(path: string) {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

export function fileMarkKind(path: string): FileMarkKind {
  return kinds[fileExtension(path)] ?? "file";
}
