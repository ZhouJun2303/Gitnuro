# AweGit Agent

## 自动提交

每次对话里只要改了仓库文件，收尾时自动提交一次。

- 只提交本次相关改动；没有改动就不提交
- 提交说明格式：`type: 描述`。类型用 `feat`、`fix`、`docs`、`style`、`refactor`、`test`、`chore`、`perf`、`art`、`revert`，一句话说明原因
- 不提交密钥、凭证、`.env`
- 不改 git config，不 force push，不 `--no-verify`，不 amend 已推送的提交

## 项目规范

客户端只在 `desktop/`。Tauri 2 + SvelteKit / Svelte 5 + TypeScript，Rust 后端。

- `desktop/src`：界面
- `desktop/src-tauri`：窗口、命令、设置
- `desktop/crates/git`：状态用 gitoxide，历史和写操作走 `git` porcelain

约定：

- 标识 `com.zhoujun.awegit`，产品名 AweGit
- 不改全局 gitconfig。仓库本地配置和 `.git/awegit` 可以写
- 写操作按仓库排队
- 前端依赖写在 `desktop/package.json`，Rust 依赖写在对应的 `Cargo.toml`
- 在 `desktop/` 测试：`cargo test -p awegit-git -- --test-threads=1`
- 在 `desktop/` 做类型检查：`pnpm check`
- 开发：`pnpm tauri dev`
- 不提交 `desktop/target`、`desktop/node_modules`、`desktop/src-tauri/target`，也不提交打包进来的 MinGit
