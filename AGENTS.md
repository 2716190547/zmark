# ZMark 全局交互约定

**适用范围：本项目所有代理，包括规划、协调、审查与执行窗口。**

- 需要用户选择或澄清需求时，默认主动调用宿主支持的原生交互式提问工具；Codex CLI 优先使用 `request_user_input`，其他宿主使用其可用的等效工具，不只在正文中列问题。
- 遵守工具的模式限制：当前模式不能使用原生提问时，说明原因并引导用户用 `/plan` 切换到 Plan 模式；写入本文件不会自动切换模式或开启功能。异步工具返回“已接受”不代表 CLI 已显示选项，不得据此宣称已成功拉起交互界面。
- 每题提供简洁的可选项，推荐项放在第一位并注明推荐理由；提醒用户可通过“其他”或自由输入表达自定义要求。
- 使用 `grill-with-docs` 时，将本轮可独立回答的问题一起拉起交互式选项；依赖尚未确定答案的问题留待下一轮。预选项不视为用户已确认。
- 交互式工具不可用时，简要说明并退回文字选项；用户明确要求纯文字时遵循用户偏好。

# ZMark 执行窗口约定

**适用范围：只约束“执行窗口”里的执行代理（例如 `w3:p8`）。** 协调窗口、规划窗口、其他工作区与人类用户不受本文件约束。

开始任务前先读本文件，不要依赖每次对话里的临时提醒。

## 本地验证、安装与最终门禁（2026-11-13 / 2026-11-18 用户更新）

- 日常代码修改和本机体验迭代直接运行 `./scripts/bundle_macos.sh --install`，构建并安装到本机供用户验收。改动过程中一律不运行测试，包括 `cargo test` 单元测试和 `scripts/dev_check.sh`，只做编译和本机安装，报告填写“未运行（按用户要求）”。测试统一放到提交推送时运行（见下条最终门禁）。用户明确要求快速检查时才使用 `scripts/dev_check.sh`。（2026-11-27 用户重申）
- 安装前若检测到 `ZMark` 正在运行，可先保存应用中的改动、优雅退出应用并提交本地工作区改动，再执行安装，无需请求用户手动退出。
- 准备推送构建更新时，在最终候选上运行一次 `scripts/full_gate.sh`，覆盖 `cargo test --workspace`、串行的 Phase H/I 六个专项和 rename 门禁。需要发布用的应用包时加 `--bundle`，并确认 `target/bundle/ZMark.app` 已生成。门禁通过后再推送。
- 最终门禁中，Phase H 的 21 项与 Phase I 的 28 项观测专项须串行运行并分别报告。Phase H B3/B4 共 8 项与默认 workspace 测试重叠，分报时注明重叠，汇总时去重。默认 workspace 测试不包含 Phase I 两组及 Phase H B1/B2。
- Phase I 两个目标由 `required-features` 隔离。最终门禁使用同一条命令运行六个目标：`cargo test -p zmark_app --features test-observation --test phase_i_syntax_and_warm_palette_test --test rework4_stage_a_workspace_test --test phase_h_b1_real_geometry --test phase_h_b2_highlight_matches_copy --test phase_h_b3_autoscroll_real_events --test phase_h_b4_cmd_c_routing -- --test-threads=1`。

## 分支与提交

- 功能开发在 `dev` 分支进行；未经明确要求不 push、不建 PR、不改写 git 历史。
- 本轮用户已于 2026-11-04 明确要求任务完成后上传 PR：由协调端在任务完成并通过自行审查和用户验收后推送交付分支、创建或更新对应 PR，附改动与验证结果并回传链接；保持 PR 打开供审阅，不自动关闭或合并。执行端继续按阶段交付回调，不提前发布未验收代码。
- 按可独立验证的阶段提交，记录实际构建、验证与未跑范围。
- 已派发任务若明确要求更严格的验证门槛，按该任务要求完成，不用新规则回溯减免。保留共享链路的有效历史回归保护。测试精简只删除有证据的重复、失效或已被更强检查替代的用例，不通过 ignore、放宽断言或缩小输入降低保护；详见 `docs/plans/test-suite-maintenance.md`。

## 开发循环与审查分级

- **测试目标结构**：Phase H/I 的 6 个命名目标（`phase_h_b1/b2/b3/b4`、`phase_i_syntax_and_warm_palette_test`、`rework4_stage_a_workspace_test`）保持独立；其余 app 测试按领域合并为 `app_state/app_editor/app_preview/app_media`，core 测试合并为 `core_all`。`hidpi_lightbox_test`、`lightbox_*` 与 `cache_accounting_test` 因进程级纹理/分配器状态保持独立。不要把这些文件并入聚合目标，也不要把命名目标并入聚合目标。
- **审查方式（2026-11-16 用户更新）**：由当前代理自行审查；不使用已弃用的 herdr-triad，不向其他 pane/窗口派审查、规划或执行任务，也不自动启动子代理。此约定覆盖旧文档及 grill-with-docs 等 skill 的独立代理审查关口；自查不表述为独立审查通过。只有用户后续明确要求协作时才另行安排。
- **任务粒度**：一次会话只推进一个大任务；跨任务拆分为独立会话或分支，避免上下文与改动范围膨胀。
- **单一写入者**：同一工作区同一时间只保留一个写入代理；批量格式化在门禁前一次完成，门禁运行期间不改源码。
- 不把 `target/`、构建产物、临时文件加入 git。
- 不使用 `git reset --hard`、`git checkout .`、`git clean -fd` 等会丢弃他人改动的命令。

## 文档所有权（2026-11-18 用户更新）

- 推送更新时必须同步检查并更新相关项目文档（如 `README.md`、`README.zh-CN.md`、`GLOSSARY.md`、`docs/releases/`、`AGENTS.md` 等），确保公开文档与最新代码、功能及约定一致。
- 未受本轮改动影响的历史 `docs/adr/` 决策记录继续保留原状，不用旧计划覆盖新约定；如发现历史文档与现行实现冲突，应在交付报告中注明或按要求一并清理。

## 协作方式

- 默认由当前代理规划、实施和自行审查，不给其他 pane 派任务。
- 旧计划中的 Planner/Reviewer/Implementer 拓扑、跨窗格回调和独立代理批准要求仅作历史记录；当前协作规则见 `docs/agent-workflow.md`。

## 架构约束

GPUI 0.2 的窗口装饰、拖拽与灯箱变换限制已记录在 `docs/adr/0029-gpui-window-chrome-and-overlay-constraints.md`。涉及这些区域的需求，先读该 ADR，不要按 Web CSS 心智模型假设。

## 完成判定

- 完成前确认：本轮改动已提交、需求逐条核对完毕；发布时最终门禁通过，本机迭代如实记录未运行测试。其他用户素材或无关改动保留并单独说明，不为得到干净状态而删除。
- 最终报告第一行写 `STATUS: DONE` 或 `STATUS: BLOCKED`，便于协调窗口判定。
- 有未完成项或替代实现时，必须写 `STATUS: BLOCKED` 或在报告中逐条标注，不能只写 DONE。

## 构建与交付

- 发布与分发规范（2026-11-18 用户更新）：执行版本发布时，不再下载校验 CI `.sha256` 文件，也不对草稿、GitHub 公开 Release 或镜像产物做回下载 SHA-256 二次比对（详见 `docs/releases/README.md`）；仅在本地为 `latest.json` / `latest-windows.json` 计算待上传产物的 SHA-256 并签名，上传后通过 API 确认资产清单完整即可。
- 构建失败不要报告完成。
- 报告改动文件、实际执行的检查和结果。已提交时附 commit hash，已运行 `cargo test --workspace` 时列出 passed/failed 数量。
- 逐条对照需求说明：已完成 / 替代实现 / 未完成。
- API 限制、已知取舍、回归风险必须显式写出。
