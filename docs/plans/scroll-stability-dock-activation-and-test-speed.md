# 分屏滚动稳定化、Dock 前置与测试提速

> 当前代理约定（2026-11-16）：herdr-triad 已弃用；由当前代理自行审查，不向其他 pane 派任务。本文跨窗格与独立代理审查要求仅作历史记录，现行规则见 [代理工作约定](../agent-workflow.md)。

状态：草稿（待独立审查）。用户已于 2026-11-06 确认以下方向：

1. **滚动修复范围**：定点稳定化，保留 size-table 虚拟列表，不重做布局。
2. **测量取舍**：内容稳定优先——可见首块像素不随实测回填移动，滚动条拇指允许被修正。
3. **Dock 场景**：窗口被其他应用挡住、窗口位于其他 Space；不涉及“已关闭/最小化”分支。
4. **测试提速**：开发循环用 cargo-nextest 并行；测试内任意迭代数改为按真实滚动范围推导 + 固定触底过冲；交付门槛仍是 `cargo test --workspace`。

## 现象

- 把预览滚动条拖到最底端后，在编辑区滚轮：预览画面与滚动条反复跳动；在预览区滚轮：画面不乱跳，但顶部内容反复出现/消失。
- 点击 Dock 图标时窗口不会前置，也不会切到窗口所在的 Space。
- 全量测试墙钟 **325.85s**（2026-11-06，热构建，54 个测试二进制顺序执行）。

## 已确认根因（代码证据）

| 编号 | 证据 | 结论 |
| :--- | :--- | :--- |
| E1 | `workspace.rs:2357-2364` 以 `recent_scroll` 传入 `preserve_ratio`；`virtual_list.rs:209-253` 在实测回填后按 `old_offset / old_max * new_max` 改写偏移 | 总高度每被实测修正一次，正文和滚动条就被按比例推一次。文档尾部未测区域估算误差最大，故触底时跳动最明显 |
| E2 | `workspace.rs:2376-2394` 每帧比较 `preview_offset` 与 `last_preview_sync_offset`，不一致就调用 `sync_preview_scroll` | E1 导致的偏移变化被误判为用户输入，回声成编辑器滚动，形成跨面板反馈环 |
| E3 | `workspace.rs:1011-1052` / `1055-1093` 读取的是事件**发生前**的偏移；GPUI 的原生滚动监听在祖先 `on_scroll_wheel` 之后注册（`gpui-0.2.2/src/elements/div.rs:2405-2453`） | 映射慢一拍；边界处“当前值 → 映射目标 → 实测改写”三者不一致 |
| E4 | 轮询 handler 从不判断源面板是否还能继续滚动 | 编辑器已触底时，每次滚轮仍把预览设到源码行映射目标，而该目标不等于真正的 `max_scroll` |
| E5 | `preview.rs:202-219` 只渲染 `visible_range`；`virtual_list.rs:108-128` 顶部不做 overdraw | E1/E3 引起偏移变化时，顶部边界块反复进出渲染范围，表现为顶部闪烁 |
| E6 | `split_scroll_sync_test.rs:48-73` 固定 400 次滚轮 × 每轮 2 次 `window.draw`；`preview_virtual_scroll_test.rs:111-128` 固定 140 次 | 单二进制约 15s；54 个二进制由 cargo 顺序启动 |
| E7 | `main.rs` 无 `cx.on_reopen`；GPUI 的 `should_handle_reopen` 仅在无可见窗口时回调（`platform/mac/platform.rs:1409-1419`），且该方法声明为无返回值；应用也未监听窗口激活 | Dock 点击后没有任何应用层逻辑把窗口前置/切 Space。GPUI 提供 `Context::observe_window_activation` 与 `Window::activate_window`，可覆盖“激活应用但窗口未前置”的场景 |

## 设计

### 1. 预览偏移恢复规则（`views/preview/virtual_list.rs`）

新增一条统一的恢复规则，替换现有 `preserve_ratio`：

1. 拖动滚动条（`drag_ratio = Some(r)`）：`offset = r * new_max`，指针优先。
2. 回填前已贴底（`old_offset >= old_max - 1.0`）：`offset = new_max`，底部钉住。
3. 其余情况：以回填前视口顶部块 `(ix, old_offset - origins[ix])` 为锚，`offset = new_origins[ix] + delta`，保证首个可见块像素不动。

- `apply_measured` 删除 `preserve_ratio` 参数，只保留 `drag_ratio`。
- `ensure_layout` 使用同一规则；AST 切换/宽度变化时贴底状态同样保持贴底。
- 结果：实测高度变化只改变表格与拇指标记，不再移动可视内容。

### 2. 滚轮投影与边界短路（`views/workspace.rs`）

- 新增纯函数 `projected_scroll_offset(current, wheel_delta_y, max)`：
  - 轮询事件先投影到“原生滚动应用后的位置”，再参与映射，消除慢一拍。
  - GPUI 偏移方向：`scroll_offset.y += delta_y`，`offset = -scroll_offset.y`，所以内容方向的投影为 `current - delta_y`，再 clamp 到 `[0, max]`。
- 编辑区滚轮：
  - 投影值等于当前值（已到顶部/底部，滚轮无法继续移动编辑器）→ 直接返回，不改预览。
  - 投影到 `editor_max` → 预览 `set_scroll_offset(preview.max_scroll())`；投影到 0 → 预览 0。
  - 其余情况用投影值对应的顶部视觉行做源码锚点映射。
- 预览区滚轮：
  - 投影值等于当前值 → 直接返回。
  - 投影到 `preview.max_scroll()` → 编辑器滚到末行；投影到 0 → 编辑器滚到首行。
  - 其余情况用投影偏移做 `block_progress_at_offset` 映射。
  - handler 不主动改写触发面板自己的偏移，原生滚动负责该面板。
- 两端都到达边界时，滚轮事件不再触碰对侧，直接消除“拖到底后在编辑区滚动导致预览反复跳”。

### 3. 回声消除（`views/workspace.rs`）

- 所有程序化预览偏移写入（实测回填、布局重建、标题跳转、自动滚动）之后同步更新 `last_preview_sync_offset`，使偏移变化不被误判为用户滚动。
- 渲染期回调仅在 `last_scroll_origin == Preview` 且距今 100ms 内执行一次，用于吸收“预览原生滚动已应用、但 wheel handler 读的是事件前偏移”的最后一拍；`Editor` 来源以及测量改写一律不触发编辑器重定位。
- 不删除 100ms 常量本身；它仍作为“最近交互”判定，而非唯一的循环防护。

### 4. Dock 窗口前置（`main.rs` + `views/workspace.rs`）

- 在窗口创建后注册 `cx.observe_window_activation`，回调记录上一帧激活状态。
- 当窗口从非激活变为激活时调用 `window.activate_window()`（macOS 下为 `makeKeyAndOrderFront:`），把已存在但被挡住/位于其他 Space 的窗口前置；对已经是前台 Key Window 的情况是幂等操作。
- 不新增平台依赖；不使用 objc 事件监听。
- 明确不在本轮范围：最后一个窗口已关闭后的 reopen、最小化窗口的 deminiaturize（用户未选择这两个场景，GPUI 0.2.2 也无对应 API）。作为已知缺口写入报告。

### 5. 测试

新增/调整（先写失败复现，再修生产代码）：

1. `preview_virtual_scroll_test.rs`：拖到最底 → 在编辑区连续滚轮向下。断言首个滚轮后预览偏移不再变化、无反向、滚动条拇指位置稳定；在文档末尾重复滚轮不产生交替值。
2. `preview_virtual_scroll_test.rs`：预览区滚轮期间，顶部首个可见块的像素原点跨测量帧保持稳定；`visible_range().start` 不来回震荡。
3. `virtual_list.rs` 单元测试：`apply_measured` 在贴底时总高度增大/缩小都保持 `new_max`；非贴底时顶部块像素锚定。
4. 现有 `split_scroll_sync_test.rs`、`editor_bottom_wheel_keeps_preview_at_end` 保持并通过。
5. Dock：把激活状态迁移抽成纯函数/方法（`should_front_window(was_active, is_active)`），用单元测试覆盖；真实前置行为列入人工验收清单（自动测试无法覆盖 NSWindow 激活）。

### 6. 测试提速

- 开发循环：`cargo nextest run --workspace`（二进制/用例并行，保留全部用例与断言）；交付门槛与 AGENTS.md 既有命令不变，仍运行 `cargo test --workspace`。
- 任意迭代数改为确定性推导：
  - 先滚到真实 `max_scroll`（上限内循环），再固定追加 40 次触底过冲事件；保留“全程不反向、触底不回退”的断言。
  - `preview_virtual_scroll_test` 的 140 次同样按 `max_scroll / delta` 推导 + 固定过冲。
  - 不删除断言、不放宽阈值、不缩小文档样本。
- 记录改动前后：全量墙钟、`split_scroll_sync_test` 与 `preview_virtual_scroll_test` 单二进制耗时。

## 验收

- 新回归测试在修复前失败、修复后通过（记录失败断言）。
- `cargo test --workspace` 全绿，记录 passed/failed/ignored 与总墙钟。
- AGENTS.md 观测命令：`cargo test -p glidemark_app --features test-observation --test phase_i_syntax_and_warm_palette_test --test rework4_stage_a_workspace_test -- --test-threads=1`。
- Phase H 21 项按既有命令串行全绿，分报。
- `./scripts/bundle_macos.sh` 更新 `/Applications/GlideMark.app`。
- 人工复现：拖预览滚动条到底 → 编辑区滚轮，画面与拇指稳定；预览区滚轮顶部不闪；点击 Dock 时被挡住/其他 Space 的窗口前置。
- 测试耗时：给出 nextest 与 `cargo test --workspace` 两组数字，分开陈述。

## 风险与取舍

- 内容锚定意味着实测修正时滚动条拇指可能移动几像素（用户已选“内容稳定优先”）。
- 贴底时总高度缩小会钉在新的 `max_scroll`，内容相对底部保持，不保证顶部块像素不变；这是“触底优先”的明确取舍。
- nextest 仅用于开发循环，不作为交付证据；其与 `cargo test` 的结果差异若出现，以 `cargo test --workspace` 为准。
- Dock 修复依赖 `windowDidBecomeKey` 触发；若人工验收仍不前置，再单独评估 objc 激活监听方案，不夹带进本轮。
- 测试迭代数改为推导值需证明仍覆盖“长时间触底过冲”，计划保留固定 40 次过冲并在计划/提交中说明理由。

## 独立审查

用户于 2026-11-06 明确要求跳过独立审查代理、由主代理自行完成（原话：“你自由选择……自由完成吧，尽量高效”）。此处记录为**用户显式豁免审查门禁**，不表述为“审查通过”，也不代替独立审查的结论。用户同时要求消除 herdr / herdr-triad 工作流对本任务的影响：本轮不执行 herdr 回调，不派 Planner/Reviewer/Implementer 三窗格。工作区中的 AGENTS.md 已同步删除子代理强制并行与 herdr 交付回调段落；该改动来自并发的用户侧清理，本计划原样保留，不重写历史。

## 实施结果

- `views/preview/virtual_list.rs`：删除 `preserve_ratio`；新增 `top_anchor`；实测回填改为“拖动比例 > 贴底保持 > 内容锚定”；新增 3 个单元测试。
- `views/workspace.rs`：编辑器/预览滚轮改为按事件投影到原生滚动后的位置；源面板到达边界时短路；两端末/首特殊映射；方向单调钳制；渲染期回声仅在 Preview 来源 100ms 内触发一次，测量改写更新 `last_preview_sync_offset`。
- `main.rs` + `views/workspace.rs` + `platform.rs`：窗口激活观察器，非激活→激活时 `activate_window()`；`should_front_window` 单元测试。
- 测试：新增“拖到预览底部后编辑区滚轮保持钉住”集成回归；`stage_a_rework_test` 的滚轮事件改为真实 1px delta 并补足可滚动尾部；格式敏感的颜色源码断言改为空白折叠比较（并发 rustfmt 暴露）。
- 门槛结果（最终候选复跑后更新）：默认全量 336 passed / 0 failed / 0 ignored；观测专项 28/0/0；Phase H 21/0/0；`cargo nextest` 执行 25.96s。
- 未完成/人工项：Dock 前置只能人工点击验证；已关闭窗口的 reopen/最小化 deminiaturize 不在范围。

## 追加设计：方向单调性

在 §2 的映射之后再加一条约束：编辑器向下滚动（`delta_y < 0`）时，预览目标偏移不得小于当前值；向上滚动时不得大于当前值。预览→编辑器的行目标同样按方向钳制。这样即使估算高度使映射目标略低于 `max_scroll`，触底后的向下滚轮也不会把预览向上拽，覆盖用户实际复现路径。

## 测试提速基线（2026-11-06，改动前）

| 方式 | 墙钟 | 结果 |
| :--- | ---: | :--- |
| `cargo test --workspace` | 325.85s | 顺序执行 54 个二进制 |
| `cargo nextest run --workspace` | **37.62s** | 331 passed / 0 failed / 0 skipped |

最慢用例：`layout_cache_refactor_test` 25.9s（30 次完全相同的重复布局调用）、`split_scroll_sync_test::editor_overscroll...` 14.95s（固定 400 次滚轮）、`preview_virtual_scroll_test::editor_bottom...` 6.44s（固定 140 次）。

精简动作（不删断言、不降阈值、不缩样本）：重复布局 30 次 → 2 次（差异由第一次增量更新已覆盖）；滚轮循环改为“滚到真实 `max_scroll` 后固定追加 40 次触底过冲”，上限沿用原次数以免死循环。

实测补充：
- 执行时间：`split_scroll_sync_test` 二进制约 18s → 6s；`preview_virtual_scroll_test` 从单个用例 6.4s 变为 3 个用例共 6.7s。
- `layout_cache_refactor_test` 25.9s → 23.7s：重复 30→2 只省约 2s，主要成本是 1M 字符文本 shaper 的首次与 fresh-cache 全量布局，样本不能缩小，保留。
- 真正的大头是源码改动后 54 个测试二进制的重新编译/链接（约 4–5 分钟），nextest 只并行执行不省构建。尝试 `[profile.test] debug = 0` 反而把该轮拖到 717s（链接 I/O 放大）且触发了一个格式敏感测试，已回退；未纳入本轮。若要继续缩短源码改动后的墙钟，下一步应合并测试目标（例如把非 AGENTS.md 指定目标的 38 个 app 测试文件合并为 3–4 个二进制），本轮不做。
