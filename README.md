# AweGit

AweGit 是 Windows 和 macOS 上的 Git 客户端。界面用 Tauri 2 和 SvelteKit，后端是 Rust。状态用 gitoxide 读取，历史和写操作走本机的 `git`。

命令都在仓库根目录执行。下面的 `pnpm` 脚本定义在 `package.json`。

## 准备

每个新终端先装好依赖，再让 Rust 链接器排到 PATH 前面。`pnpm dev` 和 `pnpm check` 不编译 Rust，可以不设这段 PATH。

```powershell
pnpm install
$env:PATH = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;$env:USERPROFILE\.cargo\bin;" + $env:PATH
```

Windows 工具链是 `nightly-x86_64-pc-windows-gnu`。WinLibs 的 `mingw64\bin` 里要有 `as.exe` 和能完成链接的 `gcc`。不要改 rustup 的全局默认工具链，也不要在仓库根目录放 `rust-toolchain.toml`。

macOS 使用系统 Git 和系统上的 `cargo`，不需要上面的 PATH。

## 快捷命令

| 要做的事 | 命令 | 结果 |
| --- | --- | --- |
| 网页上看界面 | `pnpm dev` | http://127.0.0.1:1420 ，示例数据 |
| 启动桌面程序 | `pnpm start` | 编译 Rust（仅第一次或改过后），并打开窗口 |
| 类型检查 | `pnpm check` | Svelte 和 TypeScript，不跑 Git |
| Rust 测试 | `pnpm test` | `awegit-git` 的全部测试，使用临时仓库 |
| 打 Windows 安装包 | `pnpm package:windows` | `target\release\bundle` 下的 exe 和 msi |
| 打 macOS 安装包 | `pnpm package:mac` | 只能在 Mac 上运行，产物在 `target/release/bundle` |

`pnpm start` 就是 `pnpm tauri dev`。`pnpm test` 就是 `cargo test -p awegit-git -- --test-threads=1`。

## 在网页上暂时看界面

```powershell
pnpm dev
```

浏览器打开 http://127.0.0.1:1420 。端口固定为 1420。这个地址已经被占用时，说明 Vite 已经在跑，直接打开页面即可，不要再起一个。

网页里没有 Tauri，界面用内置示例数据。可以看布局、主题和字号。Fetch、提交、打开仓库这些操作只在桌面窗口里可用。改 `src/` 后页面会自己刷新，不必重编 Rust。

`pnpm build` 只构建网页。`pnpm preview` 预览构建结果，同样是示例数据。

## 启动桌面程序

```powershell
pnpm start
```

第一次会编译 Tauri，之后改界面走已经打开的 Vite，不必重编 Rust。改了 `src-tauri` 或 `crates/git` 再执行一次 `pnpm start`。

调试程序是 `target\debug\awegit.exe`，它加载 http://127.0.0.1:1420 。单独双击它之前要先有 `pnpm dev`，否则窗口里是无法连接。`pnpm start` 会自己拉起 Vite 和窗口。

窗口已经开着时再执行 `pnpm start` 会因为 1420 被占用而失败。关掉窗口，或继续用已经打开的那个。

设置在 Windows 的 `%APPDATA%\AweGit\settings.json`，macOS 的 `~/Library/Application Support/AweGit/settings.json`。签名口令和 HTTPS 密码不写入这个文件。

重新链接桌面程序前先关掉 `awegit.exe`。它占着 `WebView2Loader.dll` 时，`cargo build -p awegit` 会失败。Vite 可以留着。

## 测试

```powershell
pnpm check
pnpm test
```

`pnpm check` 查界面类型。`pnpm test` 跑状态、差异、提交、变基、贮藏这些 Git 行为，仓库都建在临时目录里。测试不打开窗口，也不要拿当前这个 AweGit 仓库去做暂存或提交实验。

只跑一个测试：

```powershell
cargo test -p awegit-git squash_replays_commits_after_the_range -- --test-threads=1
```

## 打包

Windows：

```powershell
pnpm package:windows
```

脚本会下载并校验 MinGit 2.56.0，放到 `src-tauri\resources\git`，然后执行 `pnpm tauri build`。这一步自己设置 WinLibs 和 cargo 的 PATH。安装包在：

- `target\release\bundle\nsis\AweGit_2.0.0_x64-setup.exe`
- `target\release\bundle\msi\AweGit_2.0.0_x64_en-US.msi`

不要提交 `target\`、`node_modules\` 和下载下来的 MinGit。

macOS 上：

```sh
pnpm package:mac
```

使用系统 Git，不把 Windows 的 MinGit 打进应用。磁盘镜像在 `target/release/bundle`。

## 窗口快捷键

下面是 Windows 上的按键。macOS 上，除了切换标签的 `Ctrl+Tab` 和 `Ctrl+Shift+Tab`，其余带 Ctrl 的组合改为 Command。方向键仍是上、下。`Escape` 关闭当前对话框。

| 操作 | 按键 |
| --- | --- |
| 刷新 | `F5` 或 `Ctrl+R` |
| 提交 | `Ctrl+Enter` |
| 提交并推送 | `Ctrl+Shift+Enter` |
| 暂存或取消暂存 | `Ctrl+Shift+S` |
| 全部暂存 | `Ctrl+Alt+Shift+S` |
| 丢弃 | `Ctrl+Shift+D` 或 `Backspace` |
| 拉取 | `Ctrl+Shift+L` |
| 推送 | `Ctrl+Shift+P` |
| 获取 | `Ctrl+Shift+F` |
| 快速获取 / 拉取 / 推送 | `Ctrl+Alt+Shift+F` / `L` / `P` |
| 贮藏 | `Ctrl+Shift+H` |
| 分支 | `Ctrl+Shift+B` |
| 标签 | `Ctrl+Shift+T` |
| 克隆 | `Ctrl+N` |
| 新建仓库 | `Ctrl+Shift+N` |
| 打开仓库 | `Ctrl+O` |
| 新标签页 | `Ctrl+T` |
| 关闭标签页 | `Ctrl+W` |
| 上一个 / 下一个标签 | `Ctrl+Shift+Tab` / `Ctrl+Tab` |
| 变更 / 历史 | `Ctrl+1` / `Ctrl+2` |
| 快速启动 | `Ctrl+P` |
| 搜索 | `Ctrl+F` |
| 设置 | `Ctrl+,` |
| 在文件管理器中显示 | `Ctrl+Alt+O` |
| 终端 | `Ctrl+Alt+T` |
| 过滤分支 | `Ctrl+Shift+A` |
| 定位当前提交 | `Ctrl+0` |
| 放大 / 缩小 | `Ctrl+=` / `Ctrl+-` |

## 下载

安装包发布在 [Releases](https://github.com/ZhouJun2303/AweGit/releases)。

## 许可证

AweGit 基于 Gitnuro 修改，以 GNU GPL v3 发布。详见 [NOTICE](NOTICE) 和 [LICENSE](LICENSE)。
