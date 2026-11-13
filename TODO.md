# TODO：项目问题与优化清单

> 最近更新：2026-11-13。性能工作的统一记录见 [项目性能优化计划](dev-docs/performance-plan.md)，包含日志复核、PERF-00—10 的实现、回归结果及 release 测量。

## 待处理

- [ ] 发布后的真实性能观测：用新的关联计时核对安装包从点击到刷新完成的等待、实际远端网络长尾和系统峰值内存。当前 release 基准为独立本地种子，不能宣称历史 TLS 失败已消失或得到完整桌面提速比例，见 [性能计划第 8 节](dev-docs/performance-plan.md)。

## 已处理

- [x] 账户页同步账号慢，且同步结果在下次启动时丢失。现在本机凭据按批并发探测，超时后结束整个子进程树；同步结果由后端在设置队列里合并保存，窗口位置改为只写几何字段，不再用旧快照覆盖账号。修复：`76eb62d8`；回归：`sync_merge_keeps_one_copy_of_each_credential`、`sync_merge_fills_profile_fields_and_clears_legacy_tokens`、`probe_targets_keep_the_target_order`、`window_bounds_save_keeps_accounts_and_other_fields`，以及 `e2e/lab.spec.ts` 的 `account sync adopts the settings the backend saved`。
- [x] 远程弹窗按地址显示协议、主机、仓库，并自动匹配已经同步的账号。多个账号时可改选。修复：`1127c87f`；回归：`e2e/lab.spec.ts` 的 `remote dialog shows the synced account for the URL`。
- [x] 打开账户页不再自动同步本机账号。只有点击「同步本机已登录的账号」才读取本机已登录账号，其中包含 codeup.aliyun.com。阿里云登录打开 https://account.aliyun.com/ ，Git 主机仍是 codeup.aliyun.com。修复：`5f313cee`；回归：`recognizes_common_hosts_and_ssh_forms`，以及 `e2e/theme-panels.spec.ts` 的 `theme panel preferences accounts`，打开后同步行不出现「正在同步」，同步按钮可点。
- [x] 首选项账户页按远程选用已登录账号。网页登录和测试连通走试验台桩，不打开浏览器、不访问网络。修复：`846cc1d5`；回归：`recognizes_common_hosts_and_ssh_forms`、`match_order_prefers_override_then_login_then_default`、`a_token_is_not_offered_to_another_host`、`legacy_token_is_not_duplicated`、`failure_text_does_not_contain_the_token`、`helper_args_leave_the_token_in_the_script`、`an_unpreferred_account_does_not_answer_an_empty_username`、`status_does_not_install_an_https_helper`，以及 `e2e/theme-panels.spec.ts` 的 `theme panel preferences accounts`、`e2e/lab.spec.ts` 的 `long dialogs keep their title and first field in view`。
- [x] Git for Windows 在文件被占用时，`checkout` 可能以退出码 0 切走分支，工作区文件仍是旧内容。现在这类命令返回占用说明，不把分支切回去。失败信息只写在 stdout 时（例如 `nothing to commit`），详情改用这段输出，最多 500 字。修复：`6a2d7f6c`；回归：`writes_name_the_process_and_explain_how_to_release_the_file`、`empty_commit_failure_includes_the_stdout_explanation`。
- [x] 变基冲突预览一次读取父提交和树，并用 `merge-tree --stdin` 批量重放，不再对每步 `read-tree` 和 `commit-tree`。冲突或结果树对不上之后的步骤仍是 conditional。5 秒和 200 步上限保持不变。修复：`6a2d7f6c`；回归：`preview_replays_a_linear_history_without_rewriting_the_work_tree`、`preview_replays_commits_even_when_final_tree_hides_a_conflict`、`frozen_preview_keeps_the_original_head_after_the_branch_moves`、`custom_merge_driver_is_never_executed_by_a_preview`、`reads_a_clean_record_and_a_conflict_record`、`merge_tree_names_the_conflict`。
- [x] 同一次打开变更时，路径、侧和全文标记都相同的重叠差异读取合并为一次 `conflict_view`。修复：`6a2d7f6c`；回归：`theme panel conflict`（Apple、经典白色、VS Code 黑色）。
- [x] 试验台用例在页面内固定语言和差异样式，不改写 `.lab/settings.json`。pushbox 每次写入唯一提交说明，`--reuse` 不用重新播种。回归：`e2e/lab.spec.ts` 里原先失败的 16 项；本次连同三套主题的冲突面板共 19 项通过。
- [x] 界面改动在 Apple、经典白色、VS Code 黑色下都测到相关面板。回归：`e2e/theme-panels.spec.ts` 的 `theme panel`，以及 `pushed commits are bold`、`merge panel opens as a standalone page`。
- [x] PERF-00—10：关联计时、写后刷新与并发读取去重、活动检查点缓存、原生忽略判断与跟踪目录监听、安全的原生自动贮藏检查、纯提交元数据读取、批量冲突元数据、8KB blob 探测、线性冲突匹配、历史分页缓存、预览限额/取消和网络失败分类均已实现。修复：本次工作区改动，拟提交 `perf: 减少后端重复读取并限制后台流程开销`；回归：`commit_metadata_does_not_walk_merge_trees`、`concurrent_reads_share_work_but_the_next_read_runs_again`、`native_ignore_checks_match_git_and_reload_rules_and_the_index`、`watch_filters_keep_tracked_files_inside_ignored_build_directories`、`conflict_blob_probe_keeps_the_prefix_boundary_and_large_object_policy`、`checkpoints_group_a_thousand_conflicts_and_preserve_each_stage`、`history_pages_reuse_summaries_and_detect_external_replacement`、`preview_lane_runs_one_job_and_keeps_only_the_latest_pending_job` 及 `e2e/performance-refresh.spec.ts`。完整测量与验证限制见 [性能计划](dev-docs/performance-plan.md)。
- [x] 流程与冲突元数据复用 Git 目录缓存，已结束流程不再读取实时检查点。提交：`599049a5`；第一轮结果见 [后端性能检查](dev-docs/backend-performance.md)，后续优化见性能计划。
- [x] 冲突后继续 rebase 时，自己的提交显示成别人的名字。现在保留原作者，只更换提交者。修复：`331b306b`；回归测试：`rebase_continue_keeps_the_original_author`。
- [x] pull 冲突时，本地原有的 `.fakeignore` 被收进自动贮藏。现在工作区根目录的 `.fakeignore` 会留在原地，不进入贮藏；已被跟踪的 `.fakeignore` 仍按普通文件处理。修复：`331b306b` 的 `crates/git/src/autostash.rs`；回归测试：`autostash_leaves_untracked_fakeignore_alone`。
- [x] 选中文件夹按 Delete 时，丢弃修改的范围不正确。现在丢弃、删除和贮藏覆盖所选文件夹内的全部文件；全选文件夹后右键按整个选择范围统计并操作。修复：`ccca0f50`、`efaa48d8`；UI 用例：`folder menu counts`。
- [x] 操作与冲突流程窗口、同步按钮状态、文件占用提示和冲突面板窄宽布局已完成，细节和验证命令见 [开发与发布](dev-docs/development.md)。

## 维护约定

- 每次只验证本次修改涉及的 UI，不跑全量场景。改到的页面要在 Apple、经典白色、VS Code 黑色下都测到受影响的面板。入口、面板名和数据复用方式见 [开发与发布](dev-docs/development.md) 的“测试”部分及 `AGENTS.md` 的“试验台”规则。
- 关闭一项时写明修复提交和回归测试名，便于以后复查。

# 测试
