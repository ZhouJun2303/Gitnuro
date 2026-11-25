# TODO

## 待处理

- [ ] 正式安装包实机复测：记录启动、点击到 Git 完成及界面刷新耗时、真实远端网络长尾和系统峰值内存；补原生窗口重启和多屏/DPI 验证。按启动测量决定是否拆分主界面及试验台入口。现有本地基准不涵盖这些结果，历史 TLS 失败也尚未用真实远端复核。

## 最近完成

- [x] 丢弃子模块时，如果子模块已经停在父仓库记录的提交上，只在该子模块内重置索引和工作区，不再把它改成游离 HEAD；未跟踪文件仍保留，提交不同时仍切到父仓库记录的提交，reflog 留下正常的 checkout 记录。回归：`discard_dirty_submodule_keeps_its_branch`、`discard_paths_restores_submodule_and_regular_file_together`、`discard_submodule_restores_checkout_without_touching_other_submodules`。修复：本条所在提交 `fix: 丢弃子模块修改时不再无故切换 HEAD`。

- [x] `9fdf8a38`：冲突预览改为偏好设置 → 差异里的开关，默认关闭。关闭时不读取冲突全文，也不在面板里展开每一块；合并窗口不受影响。回归：`missing_conflict_preview_defaults_off_and_keeps_an_explicit_choice`，`e2e/conflict-preview-setting.spec.ts`。

- [x] `f4b00316`、`b5023ddf`：新增 Killer 推送开关。直接推送成功仍只执行一条 `git push`；被远程新提交拒绝或 `--force-with-lease` 过期时，自动拉取并重试一次；合并或变基冲突会中止并恢复自动贮藏，本地分支与工作区保持原样。开启时推送图标变为实心。回归：`killer_push.rs` 的 `killer_push_merges_remote_commits_then_pushes_once`、`killer_push_aborts_a_conflicting_merge_and_keeps_local_history`、`killer_push_aborts_a_conflicting_rebase_and_keeps_local_history`、`killer_push_restores_uncommitted_changes_after_a_conflicting_pull`、`killer_push_off_keeps_the_plain_rejection`、`killer_push_recovers_a_stale_force_with_lease`、`push_ref_killer_replaces_a_commit_push_with_the_integrated_branch`；`e2e/killer-push.spec.ts`。

- [x] `b6a6f53f`：工作区项目标签改用指针拖动，顺序随工作区保存；字号以界面字号为上限（放得下就用界面字号），放不下才按工作区逐级缩到 10px，仍放不下显示省略号。回归：`e2e/workspace-files.spec.ts` 的 `project tabs keep the dragged order in the workspace`、`project names shrink with the tab and ellipsize at the minimum`。

- [x] `d932b503`：全部提交搜索保留整表，匹配项用微光标出，上一个/下一个显示总数并滚到对应提交；Reveal in AweGit 滚到该提交。回归：`e2e/history-search.spec.ts`。

- [x] `258d5141`：入队校验按仓库隔离；暂存失败回读真实状态，撤销依赖失败结果的暂存请求。回归：`slow_admission_only_delays_its_own_repository`、`e2e/stage-queue.spec.ts` 的 `a failed write restores confirmed state, cancels dependent intents and allows retry`。
- [x] `296533fd`：写入保序，提交、丢弃、提交后推送核对原意图；增量回传任务变化，修复 Windows 文件占用提示。回归：`queued_discard_preserves_later_disk_and_index_changes`、`queued_commit_does_not_include_paths_staged_after_confirmation`、`dependent_push_uses_the_confirmed_commit_and_rejects_a_moved_head`、`task_deltas_bound_targets_and_omit_unchanged_entries`、`writes_name_the_process_and_explain_how_to_release_the_file`（正常 Windows 权限下单独通过）。

[队列约定](dev-docs/operation-responsiveness.md) · [性能记录](dev-docs/performance-plan.md) · [UI 与队列验收](dev-docs/ui-audit.md#2026-11-18-操作队列修复复验对照-bd9eba6e)。更早已完成事项见 Git 历史。
