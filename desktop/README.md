# AweGit desktop

Tauri 2 + SvelteKit shell. This window is a static layout. Git commands are not wired yet.

```
cd desktop
pnpm install
pnpm tauri dev
```

`cargo` must be on `PATH` (`%USERPROFILE%\.cargo\bin`). The first Rust build compiles Tauri and takes several minutes. Later UI edits reload through Vite and do not rebuild Rust.

To look at the layout without the native window, run `pnpm dev` and open http://127.0.0.1:1420. `pnpm tauri dev` needs the MSVC linker. The machine default here is `nightly-x86_64-pc-windows-gnu`, and that toolchain's `dlltool` cannot finish a Windows link.
