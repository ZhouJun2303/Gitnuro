# AweGit

AweGit 是 Windows 和 macOS 上的 Git 客户端。代码在 `desktop/`，用 Tauri 2、SvelteKit 和 Rust。状态用 gitoxide 读取，历史和写操作走本机的 `git`。

## 开发

```
cd desktop
pnpm install
pnpm tauri dev
```

在 `desktop/` 里测试：`cargo test -p awegit-git -- --test-threads=1`。类型检查：`pnpm check`。

Windows 安装包用 `desktop/scripts/package-windows.ps1`。macOS 用 `desktop/scripts/package-mac.sh`，并使用系统 Git。

## 下载

安装包发布在 [Releases](https://github.com/ZhouJun2303/AweGit/releases)。

## 许可证

AweGit 基于 Gitnuro 修改，以 GNU GPL v3 发布。详见 [NOTICE](NOTICE) 和 [LICENSE](LICENSE)。
