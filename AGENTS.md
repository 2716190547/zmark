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

## 分支与提交

- 功能开发在 `dev` 分支进行；未经明确要求不 push、不建 PR、不改写 git 历史。
- 本轮用户已于 2026-11-04 明确要求任务完成后上传 PR：由协调端在任务完成并通过独立验收后推送交付分支、创建或更新对应 PR，附改动与验证结果并回传链接；保持 PR 打开供审阅，不自动关闭或合并。执行端继续按阶段交付回调，不提前发布未验收代码。
- 按可独立验证的阶段提交；中间提交前运行本次改动及调用链上受影响功能的相关测试，记录所跑命令与未跑范围，不将有限验证提交报告为阶段完成。
- 阶段提审、打包前，在最终候选上运行 `cargo test --workspace` 及项目要求的 feature 专项回归，全绿后再提交交付。共享 editor/preview/workspace/theme/AST/布局链路的修改须包含相关历史回归；影响范围不明确时扩大到全量。
- Phase H 当前 21 项专项按既有命令串行运行（`--test-threads=1`）；其中 B3/B4 共 8 项与默认全量重叠，结果分报、去重，不把默认全量当作覆盖所有专项。
- Phase I 的 `phase_i_syntax_and_warm_palette_test` 与 `rework4_stage_a_workspace_test` 已用 `required-features` 隔离观测，阶段交付必须另跑 `cargo test -p zmark_app --features test-observation --test phase_i_syntax_and_warm_palette_test --test rework4_stage_a_workspace_test -- --test-threads=1`，并单列结果；默认全量不包含这两组，不能漏跑后声称完整回归。
- 已派发任务若明确要求更严格的验证门槛，按该任务要求完成，不用新规则回溯减免。测试精简只删除有证据的重复、失效或已被更强检查替代的用例，不通过 ignore、放宽断言或缩小输入降低保护；详见 `docs/plans/test-suite-maintenance.md`。
- 不把 `target/`、构建产物、临时文件加入 git。
- 不使用 `git reset --hard`、`git checkout .`、`git clean -fd` 等会丢弃他人改动的命令。

## 文档所有权

- 执行代理只改代码与测试，不改 `README.md`、`GLOSSARY.md`、`docs/adr/`。
- ADR 状态、索引、术语表由协调者在验收后统一更新。
- 发现文档与代码冲突时，在最终报告中列出，不自行改文档掩盖。

## 协作方式

- 根据任务需要协作，不固定角色或窗格拓扑。
- 如有委派，负责整合变更并完成适用的验证；子任务按实际边界拆分。

## 架构约束

GPUI 0.2 的窗口装饰、拖拽与灯箱变换限制已记录在 `docs/adr/0029-gpui-window-chrome-and-overlay-constraints.md`。涉及这些区域的需求，先读该 ADR，不要按 Web CSS 心智模型假设。

## 完成判定

- 完成前确认：`git status` 干净、全量测试通过、需求逐条核对完毕。
- 最终报告第一行写 `STATUS: DONE` 或 `STATUS: BLOCKED`，便于协调窗口判定。
- 有未完成项或替代实现时，必须写 `STATUS: BLOCKED` 或在报告中逐条标注，不能只写 DONE。

## 构建与交付

- 全量测试通过后运行 `./scripts/bundle_macos.sh`，确认 `/Applications/ZMark.app` 已更新。
- 构建失败不要报告完成。
- 报告 commit hash、改动文件、`cargo test --workspace` 的 passed/failed 数量。
- 逐条对照需求说明：已完成 / 替代实现 / 未完成。
- API 限制、已知取舍、回归风险必须显式写出。
