# AweGit

AweGit 是 Windows 和 macOS 上的 Git 客户端。界面用 Tauri 2 和 SvelteKit，后端是 Rust。状态用 gitoxide 读取，历史和写操作走本机的 `git`。

## 开发

```
pnpm install
pnpm tauri dev
```

`cargo` 要在 `PATH` 上（`%USERPROFILE%\.cargo\bin`）。第一次会编译 Tauri。之后改界面走 Vite，不必重编 Rust。

Windows 上用 `nightly-x86_64-pc-windows-gnu`。链接需要 WinLibs 的 `mingw64\bin`（`BrechtSanders.WinLibs.POSIX.UCRT`）排在 PATH 最前面，那里有 `as.exe` 和能完成链接的 `gcc`。不要改 rustup 的全局默认工具链，也不要在仓库根目录放 `rust-toolchain.toml`。

只看界面时运行 `pnpm dev`，打开 http://127.0.0.1:1420。调试版程序加载同一个地址，所以要先启动 Vite。

测试：`cargo test -p awegit-git -- --test-threads=1`。类型检查：`pnpm check`。

Windows 安装包用 `scripts/package-windows.ps1`，会下载并校验 MinGit 2.56.0。macOS 用 `scripts/package-mac.sh`，并使用系统 Git。

设置保存在 Windows 的 `%APPDATA%\AweGit\settings.json`，macOS 的 `~/Library/Application Support/AweGit/settings.json`。签名口令和 HTTPS 密码不写入这个文件。

## 下载

安装包发布在 [Releases](https://github.com/ZhouJun2303/AweGit/releases)。

## 许可证

AweGit 基于 Gitnuro 修改，以 GNU GPL v3 发布。详见 [NOTICE](NOTICE) 和 [LICENSE](LICENSE)。
