# AweGit desktop

Tauri 2 + SvelteKit shell. Status comes from gitoxide. History, branches, and writes go through the `git` on `PATH`.

```
cd desktop
pnpm install
pnpm tauri dev
```

`cargo` must be on `PATH` (`%USERPROFILE%\.cargo\bin`). The first Rust build compiles Tauri and takes several minutes. Later UI edits reload through Vite and do not rebuild Rust.

The host toolchain is `nightly-x86_64-pc-windows-gnu`. Rustc calls its own `dlltool`, and that `dlltool` runs GNU `as`, which rustup does not ship. The self-contained `gcc` also has no CRT objects, so it cannot finish a link. Put WinLibs `mingw64\bin` (`BrechtSanders.WinLibs.POSIX.UCRT`) on `PATH` ahead of other MinGW bins. That directory supplies `as.exe` and a `gcc` that can link. Do not change the global rustup default, and do not add a `rust-toolchain.toml` at the repository root.

To look at the layout without the native window, run `pnpm dev` and open http://127.0.0.1:1420. The browser keeps the sample repository. The debug executable loads that same URL, so it shows a connection error until `pnpm dev` is running.

The window covers the current Git client: history, branches, remotes, tags, stash, merge, rebase, reset, cherry-pick, revert, line and hunk staging, commit search, and the same shortcuts as the existing app. On macOS those shortcuts use Command where the old client does. Settings are saved to `%APPDATA%\AweGit\settings.json` on Windows (`~/Library/Application Support/AweGit/settings.json` on macOS) and are not the old DataStore. A signing passphrase and HTTPS password are not written into that file.

Git is the `git` on `PATH`, unless `git\cmd\git.exe` sits next to `awegit.exe` or inside the bundle's `resources\git\cmd`. `scripts\package-windows.ps1` downloads Git for Windows MinGit 2.56.0, checks its SHA-256, and builds the installer. `scripts\package-mac.sh` is the Mac build and uses the system Git. This Windows machine cannot produce the Mac package.
