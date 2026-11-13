# AweGit Agent

## 自动提交

每次对话里只要改了仓库文件，收尾时自动提交一次。

- 只提交本次相关改动；没有改动就不提交
- 提交说明格式：`type: 描述`。类型用 `feat`、`fix`、`docs`、`style`、`refactor`、`test`、`chore`、`perf`、`art`、`revert`，一句话说明原因
- 不提交密钥、凭证、`.env`
- 不改 git config，不 force push，不 `--no-verify`，不 amend 已推送的提交

## 项目规范

客户端在仓库根目录。Tauri 2 + SvelteKit / Svelte 5 + TypeScript，Rust 后端。

- `src`：界面。组件、状态、样式，试验台场景在 `src/lib/lab`
- `src-tauri`：窗口、命令、设置
- `crates/git`：状态用 gitoxide，差异文本、历史和写操作走 `git` porcelain；测试在 `crates/git/tests`
- `e2e`：试验台 UI 用例，按场景文件放在这里
- `scripts`：测试、种子、开发和打包脚本
- `docs`：发布到 GitHub Pages 的功能介绍页和截图
- `dev-docs`：开发文档、专题记录和计划，不发布
- `static`：前端静态资源
- `.github`：Issue 模板和 CI 工作流
- `.lab`：本地试验数据，不提交
- `build`、`.svelte-kit`、`target`、`src-tauri/target`、`test-results`、`node_modules`：生成目录，不提交

约定：

- 标识 `com.zhoujun.awegit`，产品名 AweGit
- `feat/dev` 是日常开发分支，功能和修复提交到这里；`release` 只放发布提交和 `latest.json`
- `release` 受 GitHub 规则集保护，只有仓库所有者能直接推送。协作者从 `feat/dev` 开分支，向 `feat/dev` 提 PR
- 协作者开 PR 时 base 选 `feat/dev`。仓库默认分支是 `release`，不选会指错
- 不改全局 gitconfig。仓库本地配置和 `.git/awegit` 可以写
- 写操作按仓库排队
- 前端依赖写在 `package.json`，Rust 依赖写在对应的 `Cargo.toml`
- 启动桌面：`pnpm start`（等同 `pnpm tauri dev`）
- 界面调试用桌面窗口的检查器：调试版启动时自动打开。正式包没有检查器
- 测试：`pnpm test`（`scripts/test-all.mjs`，依次跑 `awegit-git` 和 `awegit --lib`，都是 `--test-threads=1`）
- 类型检查：`pnpm check`
- Windows 打包：`pnpm package:windows`
- Windows 发布：`publish.bat`（默认定修订号加一后打包并上传），在 `feat/dev` 上运行，脚本会合并进 `release`。`pnpm publish:windows` 只在 `release` 分支发布当前版本
- macOS 打包：`pnpm package:mac`
- 不提交 `target/`、`node_modules/`、`src-tauri/target/`，也不提交打包进来的 MinGit

## 文档

`docs/` 只放发布到 GitHub Pages 的功能介绍页和截图，不放 Markdown。开发、性能、审计等内部文档一律放 `dev-docs/`。根目录只放 `README.md`、`AGENTS.md`、`TODO.md`。

- `README.md`：面向用户。功能、下载安装、常用命令、分支与发布；中文、英文两节同步改
- `AGENTS.md`：面向代理和协作者的仓库规则。只写约定，不写进度
- `TODO.md`：当前问题和优化清单。新任务写进“待处理”，关闭时写明修复提交和回归测试名
- `docs/index.html`：功能介绍页，中英文同步改；截图放 `docs/screenshots/`，引用用相对路径
- `dev-docs/development.md`：开发环境、测试、试验台、打包、发布和快捷键
- `dev-docs/*.md`：专题文档，文件名写主题，如 `performance-plan.md`。一次性的检查、审计、测量记录在标题写明日期和对照提交
- 新增 `dev-docs/*.md` 时在 `README.md` 或 `TODO.md` 加链接，不留孤儿文档
- 不要往 `docs/`、`src/`、`src-tauri/`、`crates/`、`scripts/`、`e2e/` 里加 Markdown
- 临时笔记、方案和进度不新建 Markdown：计划写进 `TODO.md` 或对应专题文档，实验数据写 `.lab/`
- 命令、约定和说明只维护一份，其他文档链接过去，不在多处重复

## 试验台

浏览器看界面用 `pnpm dev`，自动化用 `pnpm test:ui`。桌面仍是 `pnpm start`。

- `.lab/` 不提交。种子脚本 `scripts/lab-seed.mjs` 和场景清单要提交
- 改了界面、弹窗或仓库状态后，补相关场景，只验证本次改动涉及的 UI，不跑全量场景。相关页面要在 Apple、经典白色、VS Code 黑色下都测到受影响的面板。命令和面板名见 `dev-docs/development.md`
- 新弹窗必须记入 `src/lib/lab/scenes.ts`。缺一项时 `pnpm check` 会失败
- 不拿 AweGit 自身仓库做暂存、提交或推送实验。试验只写 `.lab`

## 性能

界面要一直能滚动、能点击。主线程上出现明显卡顿就是缺陷。

- 滚动和输入只处理窗口里的行。滚动位置一帧最多写一次
- 提交图、搜索索引、整表比较跟着数据变，不跟着每次重绘。历史变长时按帧往后续，不要在一次渲染里算完
- 刷新前后按字段比较。不要 `JSON.stringify` 整份状态、历史或差异；没变就留着原来的对象
- 差异有行数上限。自动换行时，屏幕外的行不要做语法高亮
- 后端可以慢，界面先保持可操作。写操作按仓库排队
