# TODO

## 待处理

- [ ] 正式安装包实机复测：记录启动、点击到 Git 完成及界面刷新耗时、真实远端网络长尾和系统峰值内存；补原生窗口重启和多屏/DPI 验证。按启动测量决定是否拆分主界面及试验台入口。现有本地基准不涵盖这些结果，历史 TLS 失败也尚未用真实远端复核。

## 最近完成

- [x] `5256d6f8`：工作区项目标签支持拖动调整顺序，顺序随工作区保存；标签名按标签宽度在 13–10px 间缩放，到最小字号后显示省略号。回归：`e2e/workspace-files.spec.ts` 的 `project tabs keep the dragged order in the workspace`、`project names shrink with the tab and ellipsize at the minimum`。

- [x] `d932b503`：全部提交搜索保留整表，匹配项用微光标出，上一个/下一个显示总数并滚到对应提交；Reveal in AweGit 滚到该提交。回归：`e2e/history-search.spec.ts`。

- [x] `258d5141`：入队校验按仓库隔离；暂存失败回读真实状态，撤销依赖失败结果的暂存请求。回归：`slow_admission_only_delays_its_own_repository`、`e2e/stage-queue.spec.ts` 的 `a failed write restores confirmed state, cancels dependent intents and allows retry`。
- [x] `296533fd`：写入保序，提交、丢弃、提交后推送核对原意图；增量回传任务变化，修复 Windows 文件占用提示。回归：`queued_discard_preserves_later_disk_and_index_changes`、`queued_commit_does_not_include_paths_staged_after_confirmation`、`dependent_push_uses_the_confirmed_commit_and_rejects_a_moved_head`、`task_deltas_bound_targets_and_omit_unchanged_entries`、`writes_name_the_process_and_explain_how_to_release_the_file`（正常 Windows 权限下单独通过）。

[队列约定](dev-docs/operation-responsiveness.md) · [性能记录](dev-docs/performance-plan.md) · [UI 与队列验收](dev-docs/ui-audit.md#2026-11-18-操作队列修复复验对照-bd9eba6e)。更早已完成事项见 Git 历史。
