# 开发效率复盘与改进

状态：已实施（用户 2026-11-07 确认直接实现并测速；按分级审查约定，本次未送独立审查）。范围只涉及测试组织、脚本与流程，不改生产行为。

## 1. 复盘数据

本次会话（2026-11-06 18:03 → 2026-11-07 10:39）共 12 个提交，覆盖改名、更新器/发布、动效、排版/设置页等多个大任务。

| 指标 | 改进前 |
| :--- | :--- |
| 测试二进制 | **53 个**（app 47 + core 10 + 命名/独立目标） |
| 源码改动后重建 | 约 **4–6 分钟**（每次源码变更重链全部目标） |
| `cargo test --workspace` | **403.56s**（其中执行约 47s，其余为编译/链接/等待） |
| `cargo nextest` 执行 | 约 **25s**（并行执行，不省构建） |
| 单次全量墙钟异常 | 一次 **2046s**，但 52 个二进制执行合计仅 47s、CPU 54s——绝大部分是机器休眠/等待，不是计算 |
| 独立审查 | **17 轮**（ZMark 方案 9、UI 4、内存/平台 4），每轮 1–8 分钟并伴随修订 |
| 其他 | 并发 rustfmt/其他代理改动导致复跑；手工截图与本地更新 E2E；`[profile.test] debug=0` 实验触发全量重建（717s，已回退） |

结论：耗时主因不是用例本身，而是 **53 个测试目标的重链**、**每个阶段都跑完整全量**、**审查轮次过多**、**单会话任务过多**，以及休眠/并发写入的空转。

## 2. 已实施改动

### 2.1 测试目标合并

- `crates/zmark_app/tests/` 下 41 个非命名测试合并为 4 个聚合目标：
  - `app_state`（20 个文件：状态、设置、侧栏、主题、导出、工作区）
  - `app_editor`（6 个：编辑器、布局缓存、选区/命中、阶段 A/B）
  - `app_preview`（8 个：预览、滚动同步、Phase J 滚动/图片）
  - `app_media`（3 个：图表缓存、图片、Mermaid 按钮）
- core 的 9 个测试合并为 `core_all`。
- **保持独立**（进程级纹理/分配器/命名目标隔离）：
  - `phase_h_b1_real_geometry`、`phase_h_b2_highlight_matches_copy`、`phase_h_b3_autoscroll_real_events`、`phase_h_b4_cmd_c_routing`
  - `phase_i_syntax_and_warm_palette_test`、`rework4_stage_a_workspace_test`
  - `hidpi_lightbox_test`、`lightbox_test`、`lightbox_color_regression_test`、`lightbox_paper_canvas_test`、`cache_accounting_test`
- 相对 `include_str!`/`include_bytes!` 路径已按新层级修正；`tests/fixtures` 保持在原位。
- 用例数、断言和 feature 门控不变；AGENTS.md 中 6 个命名目标的命令保持可用。

### 2.2 脚本

- `scripts/dev_check.sh`：`cargo nextest run --workspace --no-fail-fast` + `check_rename.sh`。日常每次改动运行。
- `scripts/full_gate.sh`：完整 `cargo test --workspace` + Phase I/H 六目标串行 + rename 门禁；`--bundle` 追加打包安装。仅最终候选运行。

### 2.3 流程（AGENTS.md）

- 日常 nextest，最终一次全量。
- 审查分级：高风险最多两轮；UI/文案/小重构不送审。
- 一次会话一个大任务。
- 同一工作区单一写入者，门禁期间不改源码。

## 3. 改进后实测

| 指标 | 改进前 | 改进后 |
| :--- | ---: | ---: |
| 测试二进制 | 53 | **12** |
| 源码改动后重建（`touch lib.rs` + `cargo test --no-run`） | 约 4–6 分钟 | **11.63s** |
| `cargo test --workspace`（热运行） | 403.56s | **46.61s** |
| `cargo nextest` 执行 | 约 25s | 25.3s（369 项） |
| `scripts/dev_check.sh`（热） | — | **26.12s** |
| `scripts/full_gate.sh`（全量 + 观测/H + rename） | 约 6–7 分钟 | **126.96s** |
| 全量结果行数 | 58 | 18 |

全部结果：369 passed / 0 failed / 0 ignored；观测 + Phase H 合计 49/0/0（分报 28 + 21）。

## 4. 风险与注意

- 合并后同二进制内的测试共享进程状态。首次合并时 `hidpi_lightbox_test` 与 `lightbox_color_regression_test` 因全局纹理/计数互相干扰而失败；已把它们与全部 lightbox 测试恢复为独立目标。以后若新增依赖进程级全局状态的测试，应单独建目标，而不是放进聚合组。
- `cache_accounting_test` 安装进程全局分配器，必须保持独立。
- 聚合目标的测试名带上模块前缀（如 `app_state::settings_modern_ui_test::...`）；按名称过滤时使用新前缀。
- `scripts/dev_check.sh` 首次运行会链接聚合目标（约 40–60s），之后热运行约 26s。
- `[profile.test] debug=0` 的旧实验不纳入本方案：那次 717s 主要来自全量重建/等待，不能据此判断稳态收益；如后续要调构建参数，需单独做前后对比。
