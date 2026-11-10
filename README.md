# AweGit

AweGit 是 Windows 和 macOS 上的 Git 客户端。界面用 Tauri 2 和 SvelteKit，后端是 Rust。状态用 gitoxide 读取，差异文本、历史和写操作走本机的 `git`。

命令都在仓库根目录执行。下面的 `pnpm` 脚本定义在 `package.json`。

## 分支与协作

- `feat/dev`：日常开发分支，功能和修复都提交到这里
- `release`：发布分支，只接收从 `feat/dev` 合并过来的发布提交，`latest.json` 也放在这个分支
- `main`、`rewrite/tauri`：历史分支，不再用于新开发

`release` 受 GitHub 规则集 “Protect Release branch” 保护：不能删除、不能强制推送，改动必须走 Pull Request，只有仓库所有者 `ZhouJun2303` 可以直接绕过规则。

发布在 `feat/dev` 上运行 `publish.bat`。脚本提交版本号，切到 `release` 合并，打包上传，最后回到 `feat/dev`。

协作者这样提交：

1. 从 `feat/dev` 拉出自己的分支，建议命名 `feat/<主题>` 或 `fix/<主题>`
2. 在自己的分支上提交并推送到 `origin`
3. 向 `feat/dev` 开 Pull Request。仓库默认分支是 `release`，开 PR 时要把 base 手动改成 `feat/dev`
4. 不直接向 `release` 提交或开 PR；发布提交和 `latest.json` 只由发布脚本更新

## 提交说明

提交框左侧是类型选择器，主题必须写成 `type: 描述`，可用类型：

`feat`、`fix`、`docs`、`art`、`style`、`refactor`、`test`、`chore`、`perf`、`revert`

在主题里直接输入 `fix: 修复登录` 时，`fix` 会自动移到左侧选择器，输入框里只留下描述；重复写 `feat: feat: 新增` 也只会保留一个类型。冒号后的空格可省，中文冒号也认。类型不符时提交按钮不可用（勾选跳过钩子时除外）。

类型列表是固定的，没有“自定义类型”设置；要加类型得同时改 `src/lib/shell/commitMessage.ts`、`crates/git/src/change.rs` 和 `src-tauri/src/ai.rs`。

## `.fakeignore`

工作区根目录放一个 `.fakeignore`，用 `.gitignore` 语法，把本地测试改动从变更列表里藏起来：

```gitignore
# 本地试验
notes/
*.local.json
```

它不改 `.gitignore`，也不改 Git 的忽略规则。默认启用；变更页的视图菜单里可以关闭，右键文件可以“加入到 fake 忽略文件”。“全部暂存”和提交前，AweGit 会把匹配的已暂存文件移出暂存区，所以不会误提交。冲突文件始终显示。

## 准备

每个新终端先装好依赖。`pnpm start`、`pnpm test`、`pnpm tauri` 和 `pnpm package:windows` 会自己把 WinLibs 和 `%USERPROFILE%\.cargo\bin` 排到 PATH 前面。`pnpm check` 不编译 Rust。

```powershell
pnpm install
```

直接在终端里跑 `cargo` 时，这个会话还要自己补 PATH：

```powershell
$env:PATH = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;$env:USERPROFILE\.cargo\bin;" + $env:PATH
```

Windows 工具链是 `nightly-x86_64-pc-windows-gnu`。WinLibs 的 `mingw64\bin` 里要有 `as.exe` 和能完成链接的 `gcc`。不要改 rustup 的全局默认工具链，也不要在仓库根目录放 `rust-toolchain.toml`。

macOS 使用系统 Git 和系统上的 `cargo`，不需要上面的 PATH。

## 快捷命令

| 要做的事 | 命令 | 结果 |
| --- | --- | --- |
| 启动桌面程序 | `pnpm start` | 编译 Rust（仅第一次或改过后），并打开窗口 |
| 类型检查 | `pnpm check` | Svelte 和 TypeScript，不跑 Git |
| Rust 测试 | `pnpm test` | `awegit-git` 和 `awegit` 的单元测试，使用临时仓库 |
| 打 Windows 安装包 | `pnpm package:windows` | `target\release\bundle` 下的 exe 和 msi |
| 发布到 GitHub | `publish.bat` | 选择版本号，打包并上传到 Release |
| 打 macOS 安装包 | `pnpm package:mac` | 只能在 Mac 上运行，产物在 `target/release/bundle` |

`pnpm start` 就是 `pnpm tauri dev`。`pnpm test` 跑 `scripts/test-all.mjs`，依次执行 `cargo test -p awegit-git` 和 `cargo test -p awegit --lib`，都加 `--test-threads=1`。

## 启动桌面程序

```powershell
pnpm start
```

第一次会编译 Tauri，之后改界面走已经打开的 Vite，不必重编 Rust。改了 `src-tauri` 或 `crates/git` 再执行一次 `pnpm start`。

界面调试用这个窗口自带的检查器。调试版启动时会自动打开。正式包没有检查器。

调试程序是 `target\debug\awegit.exe`，它加载 http://127.0.0.1:1420 。单独双击它之前要先有开发服务，否则窗口里无法连接。`pnpm start` 会自己拉起页面服务和窗口。

1420 上已经有页面服务时，再执行 `pnpm start` 会沿用它并打开窗口。这个端口被占用后再另起一个开发服务会失败。

设置在 Windows 的 `%APPDATA%\AweGit\settings.json`，macOS 的 `~/Library/Application Support/AweGit/settings.json`。签名口令只留在当前进程里，HTTPS 密码交给 Git 的凭据存储。API key、代理密码和各平台 token 放在系统凭据库（Windows 凭据管理器、macOS 钥匙串），配置文件里这些字段是空的。

重新链接桌面程序前先关掉 `awegit.exe`。它占着 `WebView2Loader.dll` 时，`cargo build -p awegit` 会失败。Vite 可以留着。

## 测试

```powershell
pnpm check
pnpm test
```

`pnpm check` 查界面类型。`pnpm test` 跑状态、差异、提交、变基、贮藏这些 Git 行为，以及设置、更新和凭据这些桌面单元测试。仓库都建在临时目录里。测试不打开窗口，也不要拿当前这个 AweGit 仓库去做暂存或提交实验。

只跑一个测试：

```powershell
cargo test -p awegit-git squash_replays_commits_after_the_range -- --test-threads=1
```

## 打包

Windows：

```powershell
pnpm package:windows
```

脚本会下载并校验 MinGit 2.56.0，放到 `src-tauri\resources\git`，然后执行 `pnpm tauri build`。这一步自己设置 WinLibs 和 cargo 的 PATH。安装包在 `target\release\bundle`，文件名里的版本号来自 `src-tauri\tauri.conf.json`。版本号以 `src-tauri/Cargo.toml` 为准，`node scripts/check-versions.mjs` 会核对安装包、清单和关于页。当前版本是 2.0.4：

- `target\release\bundle\nsis\AweGit_2.0.4_x64-setup.exe`
- `target\release\bundle\msi\AweGit_2.0.4_x64_en-US.msi`

发布到 GitHub 时，在仓库根目录双击或运行 `publish.bat`。第 1 项会把修订号加一（2.0.0 变成 2.0.1），然后打包并上传。也可以选次版本、主版本、指定版本、不改版本，或只打包不上传。

在 `feat/dev` 上发布时，脚本会先提交版本号，再切到 `release` 合并这个分支，打包上传后回到 `feat/dev`。从其他分支发布也走同一条合并路径。工作区要是干净的，并且 `gh` 已经登录。

已经在 `release` 分支、并且不想改版本号时，仍可以用 `pnpm publish:windows`。

不要提交 `target\`、`src-tauri\target\`、`node_modules\` 和下载下来的 MinGit。

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
| 全部暂存或全部取消暂存 | `Ctrl+Alt+Shift+S` |
| 丢弃修改 | `Delete`、`Ctrl+Shift+D` 或 `Backspace` |
| 全选当前列表或差异，再按一次取消 | `Ctrl+A` |
| 粘贴补丁 | `Ctrl+Shift+V` |
| 获取 | `Ctrl+Shift+F` 或 `Ctrl+Alt+Shift+F` |
| 拉取 | `Ctrl+Shift+L` 或 `Ctrl+Alt+Shift+L` |
| 推送 | `Ctrl+Shift+P` 或 `Ctrl+Alt+Shift+P` |
| 贮藏 | `Ctrl+Shift+H` |
| 分支 | `Ctrl+Shift+B` |
| 标签 | `Ctrl+Alt+T` |
| 克隆 | `Ctrl+N` |
| 新建仓库 | `Ctrl+Shift+N` |
| 打开仓库 | `Ctrl+O` 或 `Ctrl+T` |
| 关闭标签页 | `Ctrl+W` |
| 重新打开关闭的标签 | `Ctrl+Shift+T` |
| 上一个 / 下一个标签 | `Ctrl+Shift+Tab` / `Ctrl+Tab` |
| 变更 / 历史 | `Ctrl+1` / `Ctrl+2` |
| 当前分支 / 所有分支 | `Ctrl+Shift+A` |
| 定位当前分支 | `Ctrl+0` |
| 快速启动 | `Ctrl+K` |
| 搜索提交 | `Ctrl+F` |
| 后退 / 前进 | `Alt+Left` / `Alt+Right` |
| 设置 | `Ctrl+,` |
| 在文件管理器中显示 | `Ctrl+Alt+O` |
| 放大 / 缩小 | `Ctrl+=` / `Ctrl+-` |

终端从仓库菜单打开，没有单独的快捷键。

## 下载

安装包发布在 [Releases](https://github.com/ZhouJun2303/Gitnuro/releases)。版本清单是 `release` 分支根目录的 `latest.json`。

## 许可证

AweGit 基于 Gitnuro 修改，以 GNU GPL v3 发布。详见 [NOTICE](NOTICE) 和 [LICENSE](LICENSE)。
