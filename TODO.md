# TODO：项目问题与优化清单

> 最近更新：2026-11-12。原先列出的 3 个问题已由后续提交修复，目前没有已知的待处理项。新发现的问题按顺序补在下面。

## 待处理

（暂无）

## 已处理

- [x] 冲突后继续 rebase 时，自己的提交显示成别人的名字。现在保留原作者，只更换提交者。修复：`331b306b`；回归测试：`rebase_continue_keeps_the_original_author`。
- [x] pull 冲突时，本地原有的 `.fakeignore` 被收进自动贮藏。现在工作区根目录的 `.fakeignore` 会留在原地，不进入贮藏；已被跟踪的 `.fakeignore` 仍按普通文件处理。修复：`331b306b` 的 `crates/git/src/autostash.rs`；回归测试：`autostash_leaves_untracked_fakeignore_alone`。
- [x] 选中文件夹按 Delete 时，丢弃修改的范围不正确。现在丢弃、删除和贮藏覆盖所选文件夹内的全部文件；全选文件夹后右键按整个选择范围统计并操作。修复：`ccca0f50`、`efaa48d8`；UI 用例：`folder menu counts`。
- [x] 操作与冲突流程窗口、同步按钮状态、文件占用提示和冲突面板窄宽布局已完成，细节和验证命令见 [开发与发布](dev-docs/development.md)。

## 维护约定

- 每次只验证本次修改涉及的 UI，补对应场景并按文件或 `--grep` 运行，不再全量验证。入口和数据复用方式见 [开发与发布](dev-docs/development.md) 的“测试”部分及 `AGENTS.md` 的“试验台”规则。
- 关闭一项时写明修复提交和回归测试名，便于以后复查。

# 测试
