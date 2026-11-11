# AweGit 开发与发布

本文件面向在本仓库开发、测试和发版的人。用户功能、下载与快捷键见 [README](../README.md)；功能介绍页是 [docs/index.html](index.html)，在线版在 <https://zhoujun2303.github.io/Gitnuro/>。

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

它不改 `.gitignore`，也不改 Git 的忽略规则。默认启用；变更页的视图菜单里可以关闭，右键文件可以“加入到 fake 忽略文件”。“全部暂存”和提交前，AweGit 会把匹配的已暂存文件移出暂存区，所以不会误提交。冲突文件始终显示。自动贮藏会保留工作区根目录的 `.fakeignore` 本身，不会把它一起收进贮藏。

## 环境准备

每个新终端先装好依赖。`pnpm start`、`pnpm test`、`pnpm tauri` 和 `pnpm package:windows` 会自己把 GCC 和 `%USERPROFILE%\.cargo\bin` 排到 PATH 前面。GCC 按 `AWE_GCC`（目录或可执行文件）、WinLibs 安装目录、已有 PATH 的顺序查找。`pnpm check` 不编译 Rust。

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
| 浏览器试验台 | `pnpm dev` | 在 1420 端口打开界面，不启动桌面窗口 |
| UI 自动化 | `pnpm test:ui` | Playwright 跑试验台场景 |
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

1420 上已经有页面服务时，`pnpm start` 和 `pnpm dev` 都会沿用并持续监测它；服务停止后自动接管，自己启动的 Vite 意外退出时也会重启。退出试验台时只停止自己启动的进程，沿用的页面服务继续运行。启动、退出原因和 Vite 输出保存在 `.lab/logs/dev-server.log`。

设置在 Windows 的 `%APPDATA%\AweGit\settings.json`，macOS 的 `~/Library/Application Support/AweGit/settings.json`。签名口令只留在当前进程里，HTTPS 密码交给 Git 的凭据存储。API key、代理密码和各平台 token 放在系统凭据库（Windows 凭据管理器、macOS 钥匙串），配置文件里这些字段是空的。

重新链接桌面程序前先关掉 `awegit.exe`。它占着 `WebView2Loader.dll` 时，`cargo build -p awegit` 会失败。Vite 可以留着。

`pnpm dev` 还会编译并启动浏览器试验台后端 `awegit-lab.exe`。Windows 上，如果旧试验台仍在运行，Cargo 替换 `target\debug\awegit-lab.exe` 时会报“拒绝访问（os error 5）”；先在旧终端按 Ctrl+C，残留进程可用 `Get-Process awegit-lab` 查到 PID 后再停止。`pnpm start` 启动的是 `awegit.exe`，因此试验台文件被占用时桌面仍可能正常启动。

## 测试

```powershell
pnpm check
pnpm test
```

`pnpm check` 查界面类型。`pnpm test` 跑状态、差异、提交、变基、贮藏这些 Git 行为，以及设置、更新和凭据这些桌面单元测试。仓库都建在临时目录里。测试不打开窗口，也不要拿当前这个 AweGit 仓库去做暂存或提交实验。

页面服务的端口复用、自动接管、重启与退出清理可单独运行 `node --test scripts/dev-server.test.mjs scripts/lab-dev.test.mjs`；这些测试在 `.lab` 中使用临时端口和模拟后端，不启动桌面程序。

每次验证只运行本次修改相关的测试，UI 不跑全量场景。改动界面时补对应的试验台场景，用测试文件和 `--grep` 选择相关用例：

```powershell
pnpm test:ui -- e2e/sync-toolbar.spec.ts
pnpm test:ui -- e2e/operation-flow.spec.ts --grep "observer automatic opening"
pnpm test:ui -- --reuse e2e/operation-flow.spec.ts --grep "observer automatic closing|observer follows conflicts"
pnpm test:ui -- --reuse e2e/conflict-pane.spec.ts --grep "conflict pane"
pnpm test:ui -- --reuse e2e/preferences.spec.ts --grep "language shortcut|preferences backdrop|appearance menu"
pnpm test:ui -- --reuse e2e/preferences.spec.ts --grep "generation button"
pnpm test:ui -- --reuse e2e/preferences.spec.ts --grep "independent windows"
pnpm test:ui -- --reuse e2e/lab.spec.ts --grep "tab context menu copies"
pnpm test:ui -- --reuse e2e/tag-order.spec.ts --grep "tags"
pnpm test:ui -- --reuse e2e/lab.spec.ts --grep "file access errors"
pnpm test:ui -- --reuse e2e/merge-window.spec.ts
pnpm test:ui -- --reuse e2e/merge-many.spec.ts
pnpm test:ui -- --reuse e2e/merge-layout.spec.ts
```

`--` 后的参数会传给 Playwright。已有 `.lab` 仓库且本次无需重建时，加 `--reuse` 可复用数据，避免重建整套试验台和清理正在使用的开发日志：`pnpm test:ui -- --reuse e2e/sync-toolbar.spec.ts`。需要恢复的仓库可先单独执行 `node scripts/lab-seed.mjs rebase conflict`。

试验台用浏览器看界面：`pnpm dev`，默认在 http://localhost:1420 。场景清单在 `src/lib/lab/scenes.ts`，新弹窗必须记入，否则 `pnpm check` 会失败。`.lab/` 不提交，种子脚本 `scripts/lab-seed.mjs` 和场景清单要提交。

用户明确要求全面检查时，运行 `pnpm test:ui -- --reuse e2e/scenes.spec.ts`。每个有效场景分别检查中文浅色 1440×900、英文深色 960×640，验证初始化、脚本异常和主界面边界，截图与启动性能记录保存在 `.lab/ui-audit/`。场景的偏好设置读写在浏览器测试中隔离；共享试验仓库的行为测试按顺序运行，避免重建仓库时另一个页面正在读取。旧的 `dialog-resolve` 已删除，文本和二进制合并统一使用独立合并窗口。检查结果见 [界面检查记录](ui-audit.md)。

已有 `pnpm dev` 服务时，可直接复用：PowerShell 中先设置 `$env:AWEGIT_UI_URL = 'http://127.0.0.1:1420'`，再执行 `pnpm exec playwright test e2e/merge-window.spec.ts`。这条命令使用现有试验台，不启动或停止开发服务。

侧栏标签按时间由近到远排列：附注标签使用标签日期，轻量标签使用提交日期；目录分组按组内最新标签排列，组内保持时间倒序。前端以线性遍历保留后端顺序，不额外按名称排序，滚动沿用虚拟列表。试验台场景：`tag-order`；上方标签用例自动重建专用仓库，也可用 `node scripts/lab-seed.mjs tags` 单独重建。

窗口位置持久化与冲突自动打开的相关 Rust 校验：

```powershell
pnpm test:app flow_window::tests
pnpm test:app flows::tests
pnpm test:app operation_observer_auto_close
```

只跑一个 Git 测试：

```powershell
cargo test -p awegit-git squash_replays_commits_after_the_range -- --test-threads=1
```

## 文件占用提示

Git 无法替换或写入工作区文件时，命令日志会逐个列出文件和原因。Windows 能识别占用程序时显示程序名与 PID，并提示保存工作、关闭文件或退出程序、在任务管理器核对残留进程，再刷新仓库状态并继续或重试操作。无法识别程序时，提示排查占用、只读属性和写权限，避免把所有访问失败都认定为占用。说明适用于各类文件及共用 Git 执行器的操作，原始 Git 错误仍完整保留并可复制。

这类重要提示在日志中使用红色边框、16px 粗体标题和 14px 正文，文件、原因及程序/PID 加粗，窄窗口中自动换行且可以滚动查看解除步骤。从顶部“文件占用 / 访问受限”提示或错误列表打开，都能看到完整说明。试验台场景：`commands-file-access`、`commands-file-access-unknown`。相关验证：`pnpm test:git locks::tests` 和上方的 `file access errors` UI 用例；Windows 程序识别测试需要可访问 Restart Manager 的正常系统权限。

## 操作与冲突流程窗口

“操作与冲突流程”仅在本次操作实际出现待解决冲突时自动打开，包括整合提交和恢复本地修改时的冲突；正常操作、普通失败、预测冲突，以及冲突已解决但尚待继续的状态都不会触发自动打开。重新打开仓库时仍有待解决冲突，会恢复对应流程窗口。偏好设置中的“操作遇到待解决冲突时自动打开流程窗口”可关闭此行为，操作历史仍可从仓库菜单手动查看。相关验证：`pnpm test:app flows::tests` 与 `pnpm test:ui -- --reuse e2e/operation-flow.spec.ts --grep "observer automatic opening"`。

“操作与冲突流程”窗口关闭时保存位置、大小和最大化状态，手动关闭和流程完成后的自动关闭都适用。下次自动弹出或查看历史时恢复，重启应用后也保留；多个仓库共享最后关闭的窗口布局。窗口布局单独存放在设置文件同目录的 `flow-window.json`，原来的显示器断开时会回到可见屏幕。

偏好设置 → 高级中的“解决所有冲突后自动关闭流程窗口”默认不勾选，旧设置文件也默认不自动关闭。启用后，等待所有冲突解决、后续操作与本地修改恢复全部完成，再于 1.5 秒后关闭；等待继续或仍有待恢复贮藏时保持打开，手动查看历史也保持打开。已打开的流程窗口在重新获得焦点时读取最新开关，关闭前再次核对设置。试验台场景：`prefs-flow`、`operation-flow-completed`；相关验证命令见上方的自动关闭用例。

获取、拉取、推送按当前仓库的实际状态启用：前台操作执行中全部禁用；合并、变基等流程未完成或仍有冲突时，获取可用，拉取和推送禁用；处理完成后恢复。工具栏、仓库菜单、右键菜单和快捷键遵循同一规则。单纯查看历史流程不会禁用这些操作。

主窗口的冲突面板会随差异区域宽度调整：窄面板中每个版本与自己的勾选框对齐，较窄时提交号另起一行，操作按钮填满可用宽度并允许换行；高度不足时可滚动访问标题和操作。试验台场景为 `conflict-compact`，相关 UI 用例覆盖 240、300、500 和 760 像素宽度及较低的面板高度。

内置合并直接打开唯一的原生 `merge` 窗口，可拖动、缩放和最大化。关闭时把物理坐标、逻辑宽高和最大化状态保存到设置目录的 `merge-window.json`，与 `flow-window.json` 分开；显示器移除或 DPI 改变后，窗口尺寸会适配可见工作区。首次使用 1280×840，最小客户区 480×360；旧浏览器记录的尺寸仅作为首次打开的兼容回退。

重复打开同仓库同文件只激活现有窗口。关闭窗口、按 Escape 或切换文件/仓库时，未应用的选择和手工草稿需要确认丢弃，默认聚焦取消；保存期间禁止关闭，失败后保留内容并显示可重试的错误。手工编辑后的预览显示实际草稿。试验台关闭合并后返回当前仓库。

合并左右内容与结果按 32 行拆块，大文件只渲染视口与少量缓冲块，单个巨大冲突和大段公共上下文也适用。高度索引处理块测量和定位；滚动状态每帧最多更新一次，选择一侧不会重建左右原始文本。验证覆盖 2.5 万行单个冲突、1000 个冲突之间的长上下文、双向滚动、键盘选择、失败重试及最小窗口布局。窗口持久化单元测试同时覆盖坏记录、负屏幕坐标、DPI 和两类窗口的独立记录；实际桌面多显示器操作仍需在 Tauri 窗口中验证。

## 语言、偏好设置与标签页

语言快捷键在首选项和输入框中也可使用，从当前显示语言切换到另一种语言并保存；也可在“外观”菜单中选择“切换语言”。首选项点击面板外空白处保持打开，通过右上角关闭按钮、“完成”或 `Escape` 关闭。试验台场景：`prefs-language`；相关验证：`pnpm test:ui -- --reuse e2e/preferences.spec.ts --grep "language shortcut|preferences backdrop|appearance menu"`。

首选项左侧按「常规 / 工作流 / 系统」分组导航，共 11 个页签：通用、工作区、差异、提交、AI、Git、集成、命令、网络、账户、高级；每个页签内按功能卡片分区。试验台按页签提供 `prefs-language`、`prefs-workspace`、`prefs-diff`、`prefs-commit`、`prefs-ai`、`prefs-git`、`prefs-integration`、`prefs-commands`、`prefs-network`、`prefs-accounts`、`prefs-advanced`；相关验证：`pnpm test:ui -- --reuse e2e/preferences.spec.ts`。

切换语言会立即同步到已打开的独立历史、文件内容、合并和操作流程窗口；也可在独立窗口按相同快捷键切换，其他窗口同步更新。同步只更新语言，保留各窗口正在查看的内容和编辑草稿。验证：`pnpm test:ui -- --reuse e2e/preferences.spec.ts --grep "independent windows"`。

项目标签右键菜单提供“拷贝项目路径”，复制该标签对应仓库的完整路径；未选中的标签和自定义名称的标签同样适用。试验台场景：`tab-copy-path`；验证：`pnpm test:ui -- --reuse e2e/lab.spec.ts --grep "tab context menu copies"`。

点击生成提交说明时，若 AI 未启用、接口地址未填写，或后端报告未配置密钥，会打开偏好设置的 AI 页签并滚动、聚焦到对应配置项；已通过环境变量配置密钥时仍可正常生成。试验台场景：`suggest-unconfigured`、`suggest-no-url`、`suggest-disabled`；验证：`pnpm test:ui -- --reuse e2e/preferences.spec.ts --grep "generation button"`。

文件夹右键菜单在文件夹内文件全部被选中时，按整个选择范围显示数量，丢弃、删除和贮藏都作用于全部所选文件；只选中文件夹本身时仍只作用于该文件夹。验证：`pnpm test:ui -- --reuse e2e/lab.spec.ts --grep "folder menu counts"`。

## 打包

Windows：

```powershell
pnpm package:windows
```

脚本会下载并校验 MinGit 2.56.0，放到 `src-tauri\resources\git`，然后执行 `pnpm tauri build`。这一步自己设置 WinLibs 和 cargo 的 PATH。安装包在 `target\release\bundle`，文件名里的版本号来自 `src-tauri\tauri.conf.json`。版本号以 `src-tauri/Cargo.toml` 为准，`node scripts/check-versions.mjs` 会核对安装包、清单和关于页。当前版本是 2.0.7：

- `target\release\bundle\nsis\AweGit_2.0.7_x64-setup.exe`
- `target\release\bundle\msi\AweGit_2.0.7_x64_en-US.msi`

发布到 GitHub 时，在仓库根目录双击或运行 `publish.bat`。第 1 项会把修订号加一（2.0.0 变成 2.0.1），然后打包并上传。也可以选次版本、主版本、指定版本、不改版本，或只打包不上传。

在 `feat/dev` 上发布时，脚本会先提交版本号，再切到 `release` 合并这个分支，打包上传后回到 `feat/dev`。从其他分支发布也走同一条合并路径。工作区要是干净的，并且 `gh` 已经登录。

发布过程中的顺序是：提交 `latest.json`，在这个清单提交上打 `v版本号` 标签并推送，上传安装包，最后推送 `release`。标签和 GitHub release 指向同一个提交；清单最后推送，避免更新检查拿到还没上传完的安装包。

已经在 `release` 分支、并且不想改版本号时，仍可以用 `pnpm publish:windows`。

不要提交 `target\`、`src-tauri\target\`、`node_modules\` 和下载下来的 MinGit。

macOS 上：

```sh
pnpm package:mac
```

使用系统 Git，不把 Windows 的 MinGit 打进应用。磁盘镜像在 `target/release/bundle`。没有 `APPLE_SIGNING_IDENTITY` 时产物未签名、未公证；设置 `AWE_REQUIRE_SIGN=1` 会要求真实签名，缺少证书时直接失败。

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
| 切换中英文 | `Ctrl+Alt+L` |
| 在文件管理器中显示 | `Ctrl+Alt+O` |
| 放大 / 缩小 | `Ctrl+=` / `Ctrl+-` |

终端从仓库菜单打开，没有单独的快捷键。
