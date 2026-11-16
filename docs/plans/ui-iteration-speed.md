# UI 迭代提速计划

> 当前代理约定（2026-11-16）：herdr-triad 已弃用；由当前代理自行审查，不向其他 pane 派任务。本文跨窗格与独立代理审查要求仅作历史记录，现行规则见 [代理工作约定](../agent-workflow.md)。

状态：讨论中。上一版已通过独立 Reviewer；用户选择分组普通 app 测试、评估 GPUI/Phase H/I 回归去留，并已选择合并 release 流程中的 H/I 专项命令。目标是减少一次 UI 改动的反馈等待，不因功能完成而丢失回归保护。

## 已确认的用户决策

- 当前主要耗时是源码改动后的编译/链接。
- 第一阶段优先把不使用 `#[gpui::test]` harness 的普通 app 测试目标按领域分成 4 组；保留所有测试用例和断言，不以删测换速度。
- 普通 UI 热迭代暂不设固定秒数目标，先测单目标编译/链接、运行和目视检查基线，再据实确定目标。
- 不需要快捷脚本；直接使用 `cargo nextest` 的目标选择/测试名过滤及 `cargo run`。
- 开发迭代可按改动范围选测试目标；阶段候选仍须执行项目规定的完整回归和打包门槛。
- 用户要求评估 Phase H/I 与 GPUI 测试门槛是否可删除；本稿不建议删除门槛或测试。用户已授权合并 H/I 命令并同步执行约定，没有授权删测或降低回归覆盖。

## 仓库证据

- `docs/plans/scroll-stability-dock-activation-and-test-speed.md` 记录了 2026-11-06 的热构建基线：`cargo test --workspace` 325.85 秒，54 个测试结果目标顺序执行；`cargo nextest run --workspace` 为 37.62 秒。另记录源码变更后测试目标重新编译/链接约 4–5 分钟，`[profile.test] debug = 0` 实测恶化至 717 秒，已回退。
- 同一计划已经用按目标滚动过冲、减少重复布局工作等方法精简过慢用例，但这些改动无法消除每个无关测试目标的重复链接成本。
- `docs/plans/test-suite-maintenance.md` 仍引用旧候选：`test_topbar_settings_button_active_state_is_false` 当前不存在；`stage_b_preview_interaction_test.rs` 已不再调用 `calculate_block_click_char_offset`。这些旧条目应从当前候选清单移除，不能当作本轮删测依据。
- B3/B4 的 8 项会在默认全量及 Phase H 串行专项中各运行一次；项目约定将默认全量与专项分别报告，且不能把默认并行结果替代专项串行结果。因此暂不移除这组重复运行。
- 当前默认验证仍有 54 个结果行；特性门控的 Phase H/Phase I 观测测试仍需独立于默认全量运行，但两组专项可用一条多目标命令完成。全量、专项、应用安装和发布分别有明确用途。
- `HEAD a4efeb1` 有 46 个 app 集成测试源文件：24 个使用 `#[gpui::test]`，22 个没有使用 GPUI 测试 harness。后 22 个均未搜索到 `crate::` 根路径引用或文件级 `#![...]`；三个显式 `TEST_LOCK`/`TEST_MUTEX` 在单独保留的 GPUI/观测目标中。当前工作区新出现的未跟踪 `settings_about_layout_test.rs` 也未使用 GPUI harness，但属于用户进行中的变更，不计入此分组基线。
- 将上述 22 个普通 app 目标由 22 个二进制归并为 4 个，默认 workspace 测试结果目标预计从 54 降至 36（减少 18 个 link target）；这是目标数量推算，不代表已测得的墙钟收益。
- Cargo 会自动发现 `tests/` 目录顶层的测试源文件。分组需将原源文件移到子目录，再由 4 个顶层 wrapper 以模块形式引用，避免旧文件继续各自生成二进制。少数源文件使用相对 `include_str!` 路径，移动后需要同步修正。
- `scripts/release.sh` 会运行交付门槛并打包/发布；`scripts/bundle_macos.sh` 会构建 Release Bundle 并安装到 `/Applications`。它们不适合每轮 UI 微调。开发运行已有 `cargo run -p zmark_app`。
- 本草案不把“文件数量减少”当成速度指标，也不假设删除测试会缩短源码变更后的链接时间。

## 建议的开发循环

1. 在分组实施前先按现有目标运行；分组后，对 UI 微调运行最贴近的领域目标，例如设置 UI 使用 `settings_chrome_test`。使用 `cargo nextest run -p zmark_app --test <target>`，需要时通过 nextest 测试名过滤器缩小执行范围。只编译/链接所选分组，不链接其他集成测试目标。
2. 用 `cargo run -p zmark_app` 启动 Debug 版本做实际目视检查。日常迭代不运行 `scripts/release.sh`，也不运行会安装到系统应用目录的 `scripts/bundle_macos.sh`。
3. `editor`、`preview`、`workspace`、`theme`、AST 或共享布局改动，按现有约定扩展到相关历史测试；影响范围无法可靠划定时跑全量。
4. 精确阶段候选仍执行 `cargo test --workspace`、Phase H/I 合并后的 `test-observation` 串行专项，以及项目规定的 Bundle/验收。对重叠结果分开报数并去重，不降低既有门槛。
5. 第一次把工作区同步到干净状态后，记录改动前基线；分组后对同一类 UI 改动重复测量。分别记录目标构建/链接、全量 `nextest`、`cargo test --workspace` 与阶段完整门槛时间，区分编译/链接、用例执行和目视检查；不声称未经复测的收益。

## UI 区域到测试目标入口

下面是手动选择的起点，不是自动受影响分析。按实际修改继续加相关目标；一旦目标没有覆盖生产路径、跑出的结果和预期不符，或无法划清共享调用链，就扩大到完整相关历史回归；仍无法确定范围时跑全量。

| UI 改动区域 | 分组后的快速目标 | 保留的独立 GPUI 目标/需要扩大的情况 |
| --- | --- | --- |
| 设置、主题、顶栏和抽屉 | `settings_chrome_test` | 该组主要锁定 token、结构约束和快捷键数据；真实设置窗口仍用 `cargo run` 目视检查。偏好持久化变化加 `workspace_state_test` |
| 编辑器、预览文字、选区和同步滚动 | `editor_preview_test` | 真实几何命中加 Phase H `phase_h_b1_real_geometry`（`test-observation`）；拖选自动滚动加 `phase_h_b3_autoscroll_real_events`；焦点/复制路由加 `stage_b_preview_interaction_test`、`phase_h_b4_cmd_c_routing` |
| 文档状态、工作区根和解析分档 | `workspace_state_test` | 实际 drawer/workspace 事件变化加独立 GPUI harness 目标，如 `drawer_search_test`、`workspace_layout_and_input_test` |
| Mermaid、灯箱状态、缓存与绘图资产 | `diagram_assets_test` | 实际预览加载、尺寸或像素变化加 `local_images_test`、`hidpi_lightbox_test`、`lightbox_color_regression_test` 等独立 GPUI 目标 |

单目标命令示例：

```bash
cargo nextest run -p zmark_app --test settings_chrome_test
```

分组实施前命令里的目标名仍使用当前独立文件名。明确涉及另一个行为时再运行对应目标；feature 门控的 Phase H/Phase I 目标仍按 `AGENTS.md` 中的现有命令与线程数要求执行。

## 测试精简与结构调整

- 第一阶段合并 22 个非 GPUI-harness app 测试目标为以下 4 个领域 wrapper；测试函数和断言原样保留：
  - `settings_chrome_test`：`asset_registration`、`clean_palette_and_code_style`、`header_drawer_polish`、`settings_modern_ui`、`settings_shortcuts`、`sidebar_two_tier_and_header_weight`、`topbar_icons_optical_balance`。
  - `editor_preview_test`：`editor_engine`、`editor_reading_enhancement`、`preview_selection`、`pure_functions_and_wrappers`、`sync_scroll_app`、`syntax_bullet_and_list_marker`、`visual_line_wrap_and_syntax`。
  - `workspace_state_test`：`app_state`、`document_lifecycle_regression`、`parse_tier`、`workspace_root`。
  - `diagram_assets_test`：`diagram_cache`、`lightbox_paper_canvas`、`lightbox`、`mermaid_icon_button`。
- 其余 24 个 GPUI harness 测试目标、10 个 `zmark_core` 集成测试目标、全部 feature 门控和 Phase H/I 命令先保持独立；若这一阶段收益不足，再根据测量另议，不在本轮扩大范围。
- 分组时将旧源文件移入测试子目录，由 wrapper 以模块方式引用。检查相对 `include_str!` 路径和 nextest 测试名变化；`CARGO_MANIFEST_DIR` 仍是 package 根目录。构建阶段确认没有旧自动发现目标或测试用例静默消失。
- 先更新测试维护文档中的过期候选与基线，重新按“所保行为、生产入口、替代检查、执行成本、决定理由”核对当前真实用例。
- **当前删除结论：暂无已证明安全、且会缩短本轮编译/链接时间的单个测试删除项。** 旧候选已失效；`settings_modern_ui_test::test_settings_source_has_no_hard_borders_on_nav_mode_and_palettes` 的源码字符串检查可能对重构敏感，也不能证明实际渲染效果，可作为后续“以生产行为/渲染证据替代实现字符串断言”的重写候选，但在替代证据建立前不删除其约束。
- `layout_cache_refactor_test` 历史热运行约 23.7 秒，使用约 1M 字符的大样本文本验证增量布局与 fresh-cache 等价。其重复相同布局已从 30 次减到 2 次；剩余成本主要是必要的大样本 shaper/layout 路径，现阶段保留输入与断言。
- B3/B4 的 8 项在全量后又随 Phase H 串行专项运行，目的是满足独立的串行观测门槛；不能因为结果重叠而删掉测试或省略专项运行。
- 如果这 4 组的实测收益不足，再另行评估是否把部分 GPUI harness 目标分组；该范围需要单独评估 GPUI 测试并发/隔离、串行专项和失败定位成本，不自动纳入本阶段。
- 不引入基于 `git diff` 的自动受影响测试分析器，不新增通用测试框架，不通过 `ignore`、放宽断言、缩小输入或延后 feature 测试来缩短耗时。

## GPUI 视觉回归与 Phase H/I 门槛去留

**结论：不能只因一次功能验收已完成就删除 GPUI 回归或 Phase H/I 门槛。** 这些测试长期保护真实渲染、事件命中、焦点、滚动、剪贴板及异步结果等产品行为；不是一次性截图验收。当前 HEAD 有 24 个 `#[gpui::test]` 集成测试目标、共 103 个 GPUI 测试用例。静态盘点了全部目标，并重点交叉核对了 Phase H/I、`stage_b_preview_interaction_test`、`editor_text_layout_and_hit_test`、`rework4_stage_a_workspace_test`；没有找到已能由更强生产路径测试完全替代、可直接删除的完整目标。其余目标只按名称和入口做过盘点，不代表已逐断言证明没有细粒度重复。

| 范围 | 独特保护 | 与默认全量重叠 | 建议 |
| --- | --- | --- | --- |
| Phase H B1/B2：13 项 | 真实非零原点/滚动几何命中，以及嵌套高亮与实际复制一致；交付记录有生产回退后对应检查失败的证据 | 默认全量不含，需 `test-observation` | 保留专项。几何/helper 与序列化单测不能替代真实生产事件路径 |
| Phase H B3/B4：8 项 | 指针静止时自动滚动、释放/失焦/文档末尾停止，以及 Split/Source/Preview/Search 焦点下的复制路由 | 8 项在默认全量又运行一次 | 目前保留；这是已证实的重复执行项，不是重复行为。后文列出低改动合并操作和需重新审议的去重方案 |
| Phase I：28 项 | 23 项 syntax/warm-palette 检查涵盖真实 editor/preview 渲染、非阻塞解析、陈旧 revision、取消、最新请求、缓存淘汰/恢复和预算；另 5 项验证 Workspace 缓存共享/隔离及编辑后的生产绘制路径 | 两目标均以 `required-features` 隔离，默认全量不含 | 保留专项；目前没有证据证明这 28 项整体已由默认 suite 覆盖 |
| I0 真实像素/预算回归 | 检查生产元素实际加载的 `RenderImage` 像素、尺寸预算及资源释放；交付记录中的“恢复错误字节通道”和“删除生产 img 元素”两种变异均令检查失败 | `lightbox_color_regression_test` 的 6 项在默认全量 | 保留默认回归；颜色 helper 不能替代真实加载/像素证据 |

H 的 21 项里，B3/B4 的 8 项本身也属于默认全量；I 的 28 项都不属于默认全量。因此删掉整组 H/I 专项且不补替代，将让默认验证失去 41 项独有检查（H 的 13 项 + I 的 28 项）。门槛只在阶段候选提审/打包前运行，不要求每次 UI 调整运行；取消它不会缩短主要的日常源码编译/链接等待。

**已选操作精简：** `scripts/release.sh` 先运行默认全量，再用同一条 feature 命令串行运行 Phase I 和 Phase H 的六个目标，统一使用 `--test-threads=1`：

```sh
cargo test -p zmark_app --features test-observation \
  --test phase_i_syntax_and_warm_palette_test \
  --test rework4_stage_a_workspace_test \
  --test phase_h_b1_real_geometry \
  --test phase_h_b2_highlight_matches_copy \
  --test phase_h_b3_autoscroll_real_events \
  --test phase_h_b4_cmd_c_routing -- --test-threads=1
```

这保留 49 项专项、仍可按目标分报，只减少一次命令入口；不缩短实际 49 项的执行时间。默认全量与 B3/B4 的 8 项重跑仍存在。

**减少 B3/B4 重复执行的候选：** 可把 B3/B4 声明为 `required-features = ["test-observation"]`，仍由 H 的串行专项运行。该方案会把 8 项从默认目标移到 feature 目标，当前 `docs/plans/test-suite-maintenance.md` 第 28 行明确禁止以“移到默认不跑的 feature”缩短运行；因此这不是现行规则允许的直接优化，必须先由用户决定是否修订该规则，再验证目标配置、覆盖统计和测量收益。现阶段不建议直接改。

重点交叉核对没有支持删除旧 GPUI 目标：`stage_b_preview_interaction_test::test_preview_wrapped_text_hit_testing` 当前调用 `calculate_text_click_char_offset`，文件注释说明 Phase H B1 覆盖真实非零原点入口，而此项额外保留软换行边界检查；它并非 `docs/plans/test-suite-maintenance.md` 所说的旧 `calculate_block_click_char_offset` 调用。B2 的纯映射/序列化检查与 H B2 的真实渲染高亮对复制路径检查分工不同。`stage_b` 的视图切换用例与 H B4 有部分重叠，但另有立即清除选区状态的断言；焦点转移和布局性能用例也没有被 H B4 替代。上述比较没有给出可删用例。

当前删除结论：没有已证明可直接删除的 GPUI 测试或 Phase H/I 专项门槛。下一轮若要做全量逐例精简审计，应继续核对各用例的生产路径、相同失败模式、替代证据与目标执行成本；不能仅凭 `phase_*` 名称、功能完成或测试“视觉”标签删掉保护。

## 操作入口取舍

不增加开发脚本、任务运行器或新的依赖。日常直接用领域目标运行 `cargo nextest`，用 `cargo run -p zmark_app` 做目视检查。

## 验收标准

- UI 微调的标准操作是：单目标测试命令 + Debug 应用目视检查；不要求每次改动都先构建、安装正式 Bundle 或运行发布脚本。
- 测试保留数量、断言和 feature 覆盖没有因第一阶段提速而减少；项目的阶段完整回归要求不变。
- 默认测试目标应按 54 → 36 个核对；实际 test case 名称/数量、passed/failed/ignored 与分组前一致，除测试入口从独立 target 变为模块路径之外不改变断言。
- 对所选 UI 目标，记录其冷/热编译、链接与执行耗时；与原有全量循环相比，报告实测节省时间，再由用户依据基线确定反馈时限，不预设固定秒数或百分比。
- 旧候选列表更新为当前代码证据。任何后续删除/合并都逐项解释被保留的行为和替代证据。

## 风险与未决事项

- 只运行一个目标可能漏掉旁路回归；通过按共享链路扩展测试、阶段全量门槛和人工目视检查控制风险。
- 测试模块分组会改变测试二进制和测试名路径；相对 `include_str!` 路径需要调整，GPUI/观测目标的隔离不能被误合并。
- 目标合并不会省去共享 app crate 的重编译，也未测得实际节省；若墙钟没有下降或隔离/失败诊断变差，应回退分组范围。
- 目视检查不能由 GPUI headless 测试替代；反过来，目视检查也不替代行为回归。
- 本草案不新增 ADR：建议均可逆，且目前没有新的领域术语需要写入 `GLOSSARY.md`。

## 独立规划审查

首轮审查提出一项实质补充：加入 UI 区域→测试目标映射、当前精简结论，并规定目标覆盖不足时扩大回归；上一版已补入。上一版 Reviewer 复审无未解决实质异议；其非阻塞建议是今后刷新旧维护文档时用带日期的补充记录保留历史基线。

用户随后选择把 22 个不使用 `#[gpui::test]` harness 的 app 测试分为 3–4 个目标，并确认不需要交脚本；之后要求评估 GPUI/Phase H/I 测试及门槛是否可删。本修订建议分组为 4 个领域目标，保留 Phase H/I 独有保护，并将 B3/B4 默认重复列为可选优化。Reviewer 对 SHA-256 `9f964009559c99f32a1407520f75ab625cf5ce7b7b1cac7db878ead65a45c194` 复审通过，无未解决实质异议；核实 41 项独有用例、合并命令的串行语义、24 个 GPUI 目标及 103 个 GPUI 用例数。用户选择仅合并 release 流程中的 H/I 专项命令；已同步脚本与当前运行约定，未移动测试源、未修改测试或 Cargo target 配置、未运行测试。
