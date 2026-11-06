// One version, taken from src-tauri/Cargo.toml, must match every place a user or the updater can see.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function read(path) {
  return readFileSync(join(root, path), "utf8");
}

const cargo = read("src-tauri/Cargo.toml").match(/^version = "(\d+\.\d+\.\d+)"/m)?.[1];
if (!cargo) {
  console.error("src-tauri/Cargo.toml has no version");
  process.exit(1);
}

const expected = [
  ["package.json", JSON.parse(read("package.json")).version],
  ["src-tauri/tauri.conf.json", JSON.parse(read("src-tauri/tauri.conf.json")).version],
  ["crates/git/Cargo.toml", read("crates/git/Cargo.toml").match(/^version = "(\d+\.\d+\.\d+)"/m)?.[1]],
  ["latest.json", JSON.parse(read("latest.json")).appVersion],
  ["README", read("README.md").match(/当前版本是 (\d+\.\d+\.\d+)/)?.[1]],
  ["about page", read("src/lib/shell/state.svelte.ts").match(/appVersion = \$state\("(\d+\.\d+\.\d+)"\)/)?.[1]],
];

const lock = read("Cargo.lock");
for (const name of ["awegit", "awegit-git"]) {
  const match = lock.match(new RegExp(`\\[\\[package\\]\\]\\r?\\nname = "${name}"\\r?\\nversion = "(\\d+\\.\\d+\\.\\d+)"`));
  expected.push([`Cargo.lock ${name}`, match?.[1]]);
}

const readme = read("README.md");
if (!readme.includes(`AweGit_${cargo}_x64-setup.exe`)) expected.push(["README nsis name", "missing"]);
if (!readme.includes(`AweGit_${cargo}_x64_en-US.msi`)) expected.push(["README msi name", "missing"]);

let bad = false;
for (const [name, version] of expected) {
  if (version !== cargo) {
    console.error(`${name} is ${version ?? "missing"}, expected ${cargo}`);
    bad = true;
  }
}
if (bad) process.exit(1);
console.log(`version ${cargo} matches`);
