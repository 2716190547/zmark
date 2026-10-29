# GlideMark 执行约定

本文件是本仓库执行代理的默认工作约定。开始任务前先读本文件，不要依赖每次对话里的临时提醒。

## 分支与提交

- 功能开发在 `dev` 分支进行；未经明确要求不 push、不建 PR、不改写 git 历史。
- 按可独立验证的阶段提交，每个提交前运行 `cargo test --workspace`，全绿再提交。
- 不把 `target/`、构建产物、临时文件加入 git。
- 不使用 `git reset --hard`、`git checkout .`、`git clean -fd` 等会丢弃他人改动的命令。

## 文档所有权

- 实现代理只改代码与测试，不改 `README.md`、`GLOSSARY.md`、`docs/adr/`。
- ADR 状态、索引、术语表由协调者在验收后统一更新。
- 发现文档与代码冲突时，先在最终报告中列出，不要自行改文档掩盖。

## 子代理并行

- 可以按需开子代理；优先用于只读审查、独立文件、独立测试，不建议多人同时改同一文件。
- 并行任务必须有清晰边界：一个子代理只读审查，另一个只改独立模块；主代理负责最终合并。
- 主代理必须自己复核子代理结论，并统一跑全量测试后再提交。
- 子代理发现 API 限制或需求冲突时，先回报主代理，不静默降级。

## GPUI 0.2 已知约束

完整决策见 `docs/adr/0029-gpui-window-chrome-and-overlay-constraints.md`。

- macOS 红绿灯位置只在创建窗口时通过 `WindowOptions.traffic_light_position` 设置，运行时无迁移 API；侧栏折叠时用右画布 76px 左安全区避让，不要宣称已移动红绿灯。
- 窗口拖拽需要显式绑定：空白区 `on_mouse_down` 中调用 `window.start_window_move()`。
- `div` 不支持 CSS `transform: matrix()`；缩放用尺寸，平移用 `relative().left()/top()`，不要按 Web CSS 心智模型实现。
- 依赖新 GPUI API 前先做最小验证；不确定就查 `~/.cargo/registry/src/.../gpui-0.2.2/` 或写最小示例。

## 交付报告

- 报告 commit hash、改动文件、`cargo test --workspace` 的 passed/failed 数量。
- 逐条对照需求说明：已完成 / 替代实现 / 未完成。
- API 限制、已知取舍、回归风险必须显式写出，禁止把替代方案描述成原方案。
- 遇到阻塞先停下报告，不要自行扩大范围或改需求。
