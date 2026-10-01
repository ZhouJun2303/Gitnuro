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

The window can fetch, pull, push, stash, merge, rebase, reset, cherry-pick, revert, tag, and read the commit graph. Settings are saved to `%APPDATA%\AweGit\settings.json` on Windows and are not the old DataStore. A Portable Git build can sit in `git\cmd\git.exe` next to `awegit.exe`; when that file exists it is preferred over `PATH`. This machine cannot produce the Mac package.
