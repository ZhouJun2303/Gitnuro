// Prepend the Windows GNU linker and cargo, then run a command.
// pnpm start / pnpm test call this so a fresh terminal does not need a manual PATH.

import { spawn } from "node:child_process";
import { statSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, join, resolve } from "node:path";
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
  if (process.env.LOCALAPPDATA) {
    extras.push(
      join(
        process.env.LOCALAPPDATA,
        "Microsoft",
        "WinGet",
        "Packages",
        "BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe",
        "mingw64",
        "bin",
      ),
    );
  }
  const cargoBin = join(homedir(), ".cargo", "bin");
  extras.push(cargoBin);

  if (!isFile(join(cargoBin, "cargo.exe"))) {
    console.error("未找到 cargo.exe。请先安装 rustup，并安装 nightly-x86_64-pc-windows-gnu。");
    process.exit(1);
  }

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
