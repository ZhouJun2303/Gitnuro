// Prepend the Windows GNU linker and cargo, then run a command.
// pnpm start / pnpm test call this so a fresh terminal does not need a manual PATH.

import { spawn } from "node:child_process";
import { readdirSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL(".", import.meta.url)), "..");
const args = process.argv.slice(2);
if (args.length === 0) {
  console.error("usage: node scripts/with-rust-path.mjs <command> [args...]");
  process.exit(1);
}

const env = { ...process.env };

if (process.platform === "win32") {
  const extras = [];
  const gccBin = findGccBin();
  if (gccBin) extras.push(gccBin);
  if (process.env.AWE_RUSTUP) extras.push(process.env.AWE_RUSTUP);
  const cargoHome = process.env.CARGO_HOME || join(homedir(), ".cargo");
  const cargoBin = join(cargoHome, "bin");
  extras.push(cargoBin);
  const cargoExe = join(cargoBin, "cargo.exe");
  const rustcExe = join(cargoBin, "rustc.exe");
  const gccExe = gccBin ? join(gccBin, "gcc.exe") : "";

  if (!isFile(cargoExe)) {
    console.error("未找到 cargo.exe。请先安装 rustup，并安装 nightly-x86_64-pc-windows-gnu。");
    console.error(`已查找 ${cargoExe}`);
    if (process.env.CARGO_HOME) console.error(`CARGO_HOME=${process.env.CARGO_HOME}`);
    if (process.env.RUSTUP_HOME) console.error(`RUSTUP_HOME=${process.env.RUSTUP_HOME}`);
    process.exit(1);
  }
  console.log(`cargo: ${cargoExe}`);
  console.log(`rustc: ${isFile(rustcExe) ? rustcExe : "(cargo 代理会再查找 rustc)"}`);
  console.log(`gcc: ${gccExe && isFile(gccExe) ? gccExe : "(未加入 PATH)"}`);

  const merged = [...extras, env.PATH ?? env.Path ?? ""].filter(Boolean).join(delimiter);
  let sawPath = false;
  for (const key of Object.keys(env)) {
    if (key.toLowerCase() === "path") {
      env[key] = merged;
      sawPath = true;
    }
  }
  if (!sawPath) env.PATH = merged;
}

const executable = resolveCommand(args[0], env);
const child = spawn(executable.command, [...executable.prefix, ...args.slice(1)], {
  stdio: "inherit",
  env,
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});

child.on("error", (error) => {
  console.error(error.message);
  process.exit(1);
});

function findGccBin() {
  const given = process.env.AWE_GCC;
  if (given) {
    if (isFile(given)) return dirname(given);
    if (isFile(join(given, "gcc.exe")) || isFile(join(given, "gcc"))) return given;
  }
  const found = [];
  const local = process.env.LOCALAPPDATA;
  if (local) {
    const packages = join(local, "Microsoft", "WinGet", "Packages");
    try {
      for (const name of readdirSync(packages)) {
        if (!name.startsWith("BrechtSanders.WinLibs")) continue;
        const bin = join(packages, name, "mingw64", "bin");
        if (isFile(join(bin, "gcc.exe"))) found.push(bin);
      }
    } catch {
      /* WinGet is not installed */
    }
  }
  for (const root of [process.env.ProgramFiles, process.env["ProgramFiles(x86)"]].filter(Boolean)) {
    const bin = join(root, "WinLibs", "mingw64", "bin");
    if (isFile(join(bin, "gcc.exe"))) found.push(bin);
  }
  return found[0] ?? null;
}

function isFile(path) {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

function resolveCommand(command, env) {
  if (process.platform === "win32" && command === "tauri") {
    const script = join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
    if (isFile(script)) return { command: process.execPath, prefix: [script] };
  }

  if (isFile(command)) return { command, prefix: [] };

  const pathValue = env.PATH ?? env.Path ?? "";
  const extensions = process.platform === "win32" ? (env.PATHEXT || ".EXE;.CMD;.BAT;.COM").split(";") : [""];
  for (const dir of pathValue.split(delimiter)) {
    if (!dir) continue;
    for (const ext of extensions) {
      const suffix = command.toLowerCase().endsWith(ext.toLowerCase()) ? "" : ext;
      const candidate = join(dir, command + suffix);
      if (isFile(candidate)) return { command: candidate, prefix: [] };
    }
  }

  return { command, prefix: [] };
}
