# AweGit Agent

## 自动提交

每次对话里只要改了仓库文件，收尾时自动提交一次。

- 只提交本次相关改动；没有改动就不提交
- 提交说明格式：`type: 描述`。类型用 `feat`、`fix`、`docs`、`style`、`refactor`、`test`、`chore`、`perf`、`art`、`revert`，一句话说明原因
- 不提交密钥、凭证、`.env`
- 不改 git config，不 force push，不 `--no-verify`，不 amend 已推送的提交

## 项目规范

客户端在仓库根目录。Tauri 2 + SvelteKit / Svelte 5 + TypeScript，Rust 后端。

- `src`：界面
- `src-tauri`：窗口、命令、设置
- `crates/git`：状态用 gitoxide，历史和写操作走 `git` porcelain

约定：

- 标识 `com.zhoujun.awegit`，产品名 AweGit
- 不改全局 gitconfig。仓库本地配置和 `.git/awegit` 可以写
- 写操作按仓库排队
- 前端依赖写在 `package.json`，Rust 依赖写在对应的 `Cargo.toml`
- 启动桌面：`pnpm start`（等同 `pnpm tauri dev`）
- 界面调试用桌面窗口的检查器：调试版启动时自动打开。正式包没有检查器
- 测试：`pnpm test`（`cargo test -p awegit-git -- --test-threads=1`）
- 类型检查：`pnpm check`
- Windows 打包：`pnpm package:windows`
- macOS 打包：`pnpm package:mac`
- 不提交 `target/`、`node_modules/`、`src-tauri/target/`，也不提交打包进来的 MinGit

## 性能

界面要一直能滚动、能点击。主线程上出现明显卡顿就是缺陷。

- 滚动和输入只处理窗口里的行。滚动位置一帧最多写一次
- 提交图、搜索索引、整表比较跟着数据变，不跟着每次重绘。历史变长时按帧往后续，不要在一次渲染里算完
- 刷新前后按字段比较。不要 `JSON.stringify` 整份状态、历史或差异；没变就留着原来的对象
- 差异有行数上限。自动换行时，屏幕外的行不要做语法高亮
- 后端可以慢，界面先保持可操作。写操作按仓库排队
