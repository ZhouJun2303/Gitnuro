# TODO：项目问题与优化清单

> 最近一次审计：2026-11-05。条目来自源码、构建配置、现有测试和实验台脚本；先处理 P0，再按 P1/P2 排期。
>
> 优先级：P0 = 阻塞核心操作或有数据风险；P1 = 发布前应处理；P2 = 可安排的质量、性能和体验改进。

## 当前验证基线

- pnpm check：通过，0 errors、0 warnings。
- pnpm test：失败。crates/git/tests/operations.rs 中 60 个测试通过，cancel_stops_a_child_process 失败；Windows 上 stop_process_tree 后 ping 仍以成功状态退出。
- cargo test -p awegit --lib -- --test-threads=1：通过，30 个 Tauri 单元测试通过；该命令目前没有被 pnpm test 覆盖。
- pnpm test:ui：未进入 Playwright 用例；scripts/lab-test.mjs 每次启动都会安装 Chromium，本机在 ms-playwright 锁目录创建时返回 EPERM。

## P1：先修复的 bug 与可靠性问题

- [x] BUG-01 P1 取消操作不能稳定终止 Windows 子进程树（crates/git/src/cli.rs:368、crates/git/tests/operations.rs:358）。用 Windows Job Object 或可验证的 taskkill 重试/等待实现进程组取消；Unix 使用独立进程组。取消后等待句柄退出、清理 PID，并增加不会误杀后续同仓库操作的测试。验收：取消 fetch、clone、历史读取和大 diff 都能在限定时间内结束，测试在 Windows/macOS 均稳定通过。
- [x] BUG-02 P1 发布版本元数据不一致。package.json、src-tauri/Cargo.toml 和 src-tauri/tauri.conf.json 是 2.0.2，latest.json 与 README 仍写 2.0.0；updates::check 因此不会给 2.0.2 显示更新。集中版本来源，发布脚本同步 latest.json、README 和安装包，并在 CI 中检查所有版本字段一致。
- [x] BUG-03 P1 设置文件写入需要崩溃安全和损坏恢复（src-tauri/src/settings.rs:570-590）。当前直接 fs::write，进程中断可能留下半个 JSON；解析失败也会使设置加载整体失败。改为临时文件写入、刷盘、原子替换并保留上一份备份；加入 schema 版本、迁移和损坏文件恢复提示。验收：写入中断、旧版本字段、空文件和非法 JSON 都不会丢失最近一次有效设置。
- [ ] BUG-04 P1 凭据不应长期明文保存在 settings.json（src-tauri/src/settings.rs:25-55,135-230、README）。API key、代理密码和各平台 token 当前写入用户配置目录。迁移到 Windows Credential Manager/macOS Keychain，配置文件只保留引用；导入、诊断、日志和错误信息继续脱敏，并补升级迁移测试。
- [ ] BUG-05 P1 更新包缺少完整性验证和平台启动分支（src-tauri/src/updates.rs:48-99）。当前只校验 GitHub URL，下载后直接启动文件，没有签名或 SHA-256 校验；macOS 的 DMG 也不能按 Windows 的方式直接启动。为每个平台发布签名/哈希，下载到临时文件后校验并原子替换；Windows 使用安装程序，macOS 使用 open，失败时清理临时文件并保留可重试状态。
- [ ] BUG-06 P1 Tauri 安全边界过宽（src-tauri/tauri.conf.json:23、src-tauri/capabilities/default.json:10-91）。CSP 为 null，opener 允许打开任意路径，多个高权限命令对所有窗口开放。配置最小 CSP，按主窗口/独立窗口拆分 capability，限制外部 URL 和可打开路径；为命令增加参数边界测试，并验证错误页面或远程内容不能调用本地命令。
- [ ] BUG-07 P1 AI 和 Forge 功能依赖外部 curl（src-tauri/src/ai.rs:292-372、src-tauri/src/forge.rs:330-370）。系统没有 curl 时功能直接失败，代理、CA 文件、SSL 开关和更新模块的 HTTP 行为也不一致。复用已存在的 ureq 客户端或可靠定位应用自带 curl，统一超时、代理、CA 和错误码；用本地测试服务器覆盖 HTTP、HTTPS、代理、重定向和无 curl 环境。

## P1：测试、构建和发布流程

- [ ] TEST-01 P1 建立跨平台 CI 门槛。github 目前只有 issue 模板，没有 Windows/macOS 构建和测试工作流。加入 pnpm check、cargo test -p awegit-git、cargo test -p awegit --lib、打包 smoke test 和实验台测试；矩阵至少覆盖 Windows x64、macOS Intel、macOS Apple Silicon，并上传失败日志和诊断信息。
- [ ] TEST-02 P1 修复实验台浏览器安装流程（scripts/lab-test.mjs:13）。测试脚本不应每次运行都修改用户级 Playwright 缓存；检测浏览器是否已安装，允许通过环境变量指定缓存目录，CI 中单独执行安装并缓存。EPERM、残留锁和缺少浏览器时输出明确修复命令。
- [ ] TEST-03 P1 扩大自动化覆盖范围。pnpm test 只运行 awegit-git，不会运行 30 个 Tauri 单元测试；补一个统一测试入口，并为设置原子写入、更新校验、取消操作、watcher 丢事件、特殊路径和 Git 版本差异补回归测试。
- [ ] REL-01 P1 固定唯一版本来源并验证发布产物。发布前自动检查 Cargo、npm、Tauri 配置、latest.json、安装包文件名和关于页版本；发布脚本失败时不应留下部分合并或错误的 latest.json。

## P1/P2：性能和资源使用

- [ ] PERF-01 P1 降低仓库监视器启动和丢事件风险（src-tauri/src/lib.rs:718-990）。打开仓库会递归扫描工作树并建立大量 watch；代码注释已指出大型仓库在 Windows 上会卡顿，而且状态读取和 watcher 建立之间存在窗口。改用 Git 感知的目录集合或分层惰性 watch，建立后立即做一次增量校验；明确只监视当前仓库还是维护每个打开 tab 的 watcher。记录建立耗时、watch 数和丢事件后的恢复次数。
- [ ] PERF-02 P1 长历史图计算移出渲染路径（src/lib/view.ts:62-270、src/lib/shell/state.svelte.ts:50,1714-1853,5655-5690）。合并折叠在 25,000 条提交以内使用位图，内存和计算量仍可能达到 O(n²/32)；sameValue 在刷新时还会逐项比较整份历史。使用提交 tip/refs generation 做增量失效，把图布局和折叠放进 worker 或后端分页计算；保留按帧分批，避免任何一次重绘超过一帧预算。
- [ ] PERF-03 P1 换行差异失去虚拟化（src/lib/shell/state.svelte.ts:1846-1852）。diffWordWrap 开启时 diffWindow 和历史差异窗口直接返回全部行，长文本会一次创建大量 DOM。实现按可见区域的换行高度估算和虚拟列表，或在超过阈值时提示切换不换行模式；验收 50,000 行文本滚动、选择和复制仍可操作。
- [ ] PERF-04 P2 让状态、统计和 LFS 信息按字段缓存（crates/git/src/ops.rs:2224-2238）。repo_facts 会并行启动 ignored、activity、LFS 和 hot-path 查询，完整统计在大仓库可能耗时数秒；前端缓存只按仓库路径区分，缺少基于 HEAD、index 和工作树变化的失效条件。拆分 brief/full 结果，按实际变更字段刷新，并在确认没有 Git LFS 时跳过 LFS 查询。
- [ ] PERF-05 P2 清理仓库队列对象（src-tauri/src/lib.rs:25-90）。repo_gates 是永不删除的全局 HashMap，长时间打开大量仓库会持续保留 gate。改用引用计数、空闲回收或有上限的缓存，并补多仓库长时间运行的内存测试。
- [ ] PERF-06 P2 仓库扫描不要静默丢失结果（crates/git/src/ops.rs:2408-2437）。扫描最多返回 300 个仓库，达到上限后没有截断标记，用户会误以为目录中只有这些仓库。返回 truncated 和继续扫描入口，或让用户设置上限并在界面明确提示。

## P1/P2：兼容性和数据完整性

- [ ] COMP-01 P1 明确并检查 Git 版本能力。Windows 打包固定 MinGit 2.56，macOS 使用系统 Git，但启动时没有最低版本/能力探测；merge-tree --write-tree、LFS、worktree 等功能在旧版 Git 上可能失败。启动时显示 Git 路径、版本和缺失能力，提供降级路径；测试系统 Git 2.39、2.40、2.56 以及没有 Git/LFS 的环境。
- [ ] COMP-02 P1 移除对固定开发机路径的依赖（scripts/with-rust-path.mjs:20-39、scripts/package-windows.ps1:32）。WinLibs 路径写死为一个 WinGet 包目录，换版本、离线环境或 ARM64 机器都会找不到工具链。支持环境变量和常见安装位置探测，输出实际使用的 cargo/rustc/gcc，并在 CI 验证干净机器构建。
- [ ] COMP-03 P1 完善 macOS/Windows 发布兼容性。macOS 打包脚本没有签名、公证、最低系统版本和 Intel/Apple Silicon 产物说明；Windows 也没有验证 WebView2 已安装或离线安装行为。定义支持的系统版本和架构，产出签名/公证包，增加干净系统安装、升级、卸载和无网络启动 smoke test。
- [ ] COMP-04 P2 支持特殊和非 UTF-8 路径（crates/git/src/change.rs:1160-1175、crates/git/src/ops.rs:1160-1180 等）。多个 name-status 和路径解析使用非 -z 输出与 String::from_utf8_lossy，包含制表符、换行或非 UTF-8 字节的文件名可能被改写、截断或无法再次操作。统一使用 NUL 分隔和字节安全的内部路径表示，界面层再做可逆显示；补 Unicode、空格、tab、换行、大小写敏感文件系统测试。
- [ ] COMP-05 P2 统一网络代理和 TLS 行为。Git 命令读取代理/CA/SSL 设置，AI 和 Forge 的 curl 路径没有完整复用这些设置。抽出共享网络配置和错误模型，验证 HTTP 代理、SOCKS、企业 CA、关闭 SSL 校验和超时行为在三类请求中一致。
- [ ] COMP-06 P2 错误信息改用稳定错误码并补齐本地化。前端目前只翻译少数固定字符串，Git、curl、系统工具的大量错误直接显示英文原文。后端返回分类错误码和安全参数，前端按语言渲染可操作提示，同时保留原始诊断信息供复制。

## 交付顺序建议

1. 先修 BUG-01、BUG-02、BUG-03，并让 pnpm test 全绿。
2. 同一迭代完成 BUG-05、BUG-06、BUG-07 和 TEST-01/02，建立可重复发布和安全边界。
3. 然后处理 PERF-01/02/03，使用大仓库基准记录首屏、刷新、滚动、内存和取消延迟。
4. 最后按 COMP-01 至 COMP-06 扩展平台矩阵、路径覆盖和本地化。

## 已处理

- 取消会停掉 Windows 作业对象里的进程树，并等到进程退出。同一次仓库操作用代际号分开，后启动的命令不会被上一次取消杀掉。读取取消不会停写操作。
- 版本以 `src-tauri/Cargo.toml` 为准，当前 2.0.2。latest.json、README、关于页回退和安装包名与它对齐；`node scripts/check-versions.mjs` 和 CI 检查这些字段。发布时安装包已上传但清单未推送，则不再回退 release 合并。
- 设置先写入临时文件并刷盘，再原子替换。能解析的当前文件会留下 `.bak`；空文件和非法 JSON 恢复这份备份，并在现有错误条说明。恢复提示不写回文件。更新的 schema 不覆盖，旧文件补上 schema 1。
- 一次丢弃多个文件，不再逐个调用。
- 克隆、更新、冲突、进行中和操作错误都放到顶部中间。
- 二级弹窗固定大、中、小三种宽度，去掉撑高的最小高度。回车确认最上层弹窗，Esc 只关最上层。
- 合并弹窗写明来源分支和当前分支，自动填提交说明和变更文件。
- `.tmp_artifact` 是 Codex 的临时目录。状态和文件监视都不再把它当成仓库改动。
- 远程没有的本地分支，在历史标签和侧栏分支右上角标出。
- 全量 UI 审计：99 个试验台场景在亮/暗/窄三组下截图与 DOM 检查；修掉 P0 的变基勾选、未解决冲突可提交、确认框层级、命令面板遮挡、合并编辑器透明令牌、hunk 浮层偏移、差异行高、历史滚动位置丢失。
- 主题改为始终解析出 light/dark，系统暗色不再漏掉语法和详情强调色；颜色、圆角、阴影、焦点环收进主题令牌。
- 弹窗统一关闭按钮、内联错误、字段标签、危险样式和焦点陷阱；偏好设置 Git 页完成分组与中文化。
- 菜单超过一屏不再压缩行高；补键盘导航和菜单角色。
- 变更列表显示文件目录、选中行自动滚入视野；历史选中整行着色，头部固定，日期按偏好格式化。
- 合并解决在仍有冲突时禁用；手动编辑结果后切换侧栏会先确认，仓库切换时关闭浮层。
- 差异字号改变时行列高度按字体计算；minimap 标记按位置分布；像素差异切换文件时重置。
- 提交详情：补回分栏 hunk 元信息行的单次渲染，统一行高按差异字号计算；去掉提交编辑器和详情面板上方与分隔条重复的横线。
- 冲突视图：副标题说明勾选/拼接规则，两侧都保留时按钮改为“保留两边并合并”；命令记录的说明允许两行，历史过滤条长路径省略。
- 空状态补齐：空 Blob 不再一直显示读取中，引用日志无记录时有说明，文件历史的年份贴边不裁切、提交点始终落在轨道内。
- 差异块新增键盘可达的导航/暂存/丢弃工具条；分栏的 hunk 元信息只渲染一行；行号栏按字符宽度显示，五位数不再截断。
- 命令记录、仓库选择器、快速启动、通知和确认框补焦点陷阱与 Esc；关闭后焦点回到触发处；菜单子菜单支持方向键打开/关闭。
- 合并概览改为“保留 {分支}”并写明拼接规则；没有选择任何一侧时禁用解决；结果区增加“编辑结果”入口；合并面板限制 Tab 循环。
- GitHub/GitLab/Bitbucket/Azure 建仓字段补标签；mailmap、自定义命令、squash、贮藏重命名和建分支补可访问名称。
- 删除账户和已合并分支先确认；Git Flow 对话框回车执行开始；通知支持全部标为已读。
- fetch/pull/push 的 prune、标签、变基和 autostash 选项即时保存；历史过滤的空状态、文件列表完整路径提示也已处理。
- 追溯行恢复按钮默认样式，修正暗色下的文字对比度；错误图标改为内联 SVG；打印时隐藏工具栏，只保留差异正文。
- 清掉已无引用的旧文案键，保留提交类型这类运行时拼出来的键。
- 对比度：浅色次级文字和详情强调蓝调深到 4.5:1 以上，提交说明输入和描述框补可访问名称，确认框/通知的来源分支/弹窗各有正确的 aria-labelledby。
- 键盘：树行、命令条目、仓库行、菜单和外层浮层补统一焦点环；640 高度下变基列表收紧，主操作按钮保持在对话框内。
- 弹窗初始焦点改为第一个可见输入框（原来选择器顺序让焦点总是落在关闭按钮）；没有输入框时才回到关闭按钮。
- 菜单方向键不再连带移动历史选择；关闭菜单或确认框后焦点回到触发控件。历史行支持空格打开，嵌套链接回车不会误触整行。
- 拖动分隔条的方向键不再连带改变历史选择；各分隔条补“调整侧栏/列表/详情”等可访问名称。
- 侧栏搜索选项支持方向键、Home/End、Esc 和焦点返回；选中后回到搜索框。最小侧栏宽度下菜单不再被裁切。
- 标签页关闭按钮在未悬停时不再吃掉点击；继续变基说明和 GitHub 平台下拉补可见字段标签。
- 99 个场景在 960×640 下无横向溢出、无控制台错误、弹窗焦点都在最上层；99 个场景可访问性扫描 0 问题。

