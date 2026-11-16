# macOS 关闭主窗口即退出

> 当前代理约定（2026-11-16）：herdr-triad 已弃用；由当前代理自行审查，不向其他 pane 派任务。本文跨窗格与独立代理审查要求仅作历史记录，现行规则见 [代理工作约定](../agent-workflow.md)。

状态：规划中（第二轮独立审查）

## 背景与事实

- 用户点击 macOS 红色关闭按钮后，Dock 图标仍在，但再次点击 Dock 无反应。
- 当前 `WorkspaceView::on_window_should_close` 调用 `AppState::handle_window_should_close`：
  - 无未保存文档时返回 `true`，GPUI 只移除窗口，不退出应用。
  - 有未保存文档时返回 `false`，弹出保存/丢弃确认；全部解决后 `after_dirty_doc_resolved` 调用 `window.remove_window()`。
- 应用没有注册 `Application::on_reopen`，所以窗口移除后 Dock 点击没有回调。
- 实测无窗口时应用 Physical Footprint 约 47MB；用户已决定不保留后台/托盘。
- 设置窗口是独立辅助窗口，没有 should-close 回调；因此本计划语义是“关闭主窗口即退出”，不是“任意最后一个窗口”。

## 已确认决策

1. 关闭主窗口时**退出应用**，Dock 图标消失。
2. 不做托盘、不做后台驻留、不实现 Dock 点击重开。
3. 未保存文档的保存/丢弃确认流程保持不变。
4. 更新重启流程不能被新的退出逻辑提前打断或残留。

## 实现方案

### 1. 退出放在 main.rs 的窗口回调层

`main.rs` 已注册：

```rust
window.on_window_should_close(cx, move |window, cx| {
    view_close.update(cx, |this, cx| this.on_window_should_close(Some(window), cx))
})
```

改为：

```rust
window.on_window_should_close(cx, move |window, cx| {
    let should_close = view_close.update(cx, |this, cx| {
        this.on_window_should_close(Some(window), cx)
    });
    if should_close {
        // Platform::quit() 在 macOS/Windows/Linux 都是异步派发，不会在
        // windowShouldClose 回调栈内同步 terminate；这里直接调用即可。
        cx.quit();
    }
    should_close
})
```

- `WorkspaceView::on_window_should_close` 保持现有语义：只判断能否关闭、处理脏文档确认，不直接退出。
- `restart_for_update` 继续调用它；干净文档时返回 `true` 后安装更新，不会被这条 main 回调路径提前 quit。
- 这样不需要拆分 `can_close_window`，改动面最小。

### 2. 脏文档解决后的退出

`after_dirty_doc_resolved` 在 `is_quitting` 且没有剩余脏文档时：

- 有 `pending_restart`：走现有 `install_prepared_update`。
- 否则调用 `cx.quit()`，不再调用 `window.remove_window()`，避免“窗口没了但进程还在”。

### 3. 修复 pending_restart 残留

审查确认两条预存在路径在本次语义下会变成错误重启：

1. `prompt_save_as_and_close` 的 Save As 对话框取消分支：
   - 当前只清 `is_quitting` 和 `pending_close`，不清 `pending_restart`。
   - 改为同时设置 `pending_restart = false`，用户取消保存就不再保留更新重启意图。
2. `install_prepared_update` 的失败分支（无 target bundle、spawn 失败）：
   - 当前保留 `pending_restart`，后续任意脏文档关闭都会再次尝试安装。
   - 改为在错误分支清 `pending_restart = false`；更新仍保留在 `prepared_update` 中，用户可手动重试。

### 4. 保持范围最小

- 不注册 `on_reopen`。
- 不新增 Preferences、i18n、托盘代码或窗口激活策略。
- 不修改 `AppState::handle_window_should_close` 的纯状态逻辑，现有测试继续有效。

## 测试与验收

### 自动测试

- 现有 `AppState::handle_window_should_close` 测试保持通过：干净文档返回 `true`，脏文档返回 `false` 并进入确认。
- 现有测试没有覆盖 `handle_modal_cancel`/`handle_modal_save`/`handle_modal_discard` 的 UI 行为；
  `state.cancel_close()` 只验证状态清理，不验证窗口保留。取消路径由手工验收覆盖；
  必要时补一个 `add_window_view` 级测试，但不把状态测试当作 UI 测试。
- 更新 `app_state_test.rs` 中仍写着 “ready to remove_window” 的过时注释/结论，改为“允许关闭，由窗口回调决定退出”。
- 新增可执行的回归（若 GPUI 测试上下文可模拟）：
  - Save As 取消后 `pending_restart == false`；
  - 安装失败后 `pending_restart == false`。
  - 若无法通过公开测试上下文观察/驱动这两个路径，则在报告中明确标注为手工验收，不伪造自动覆盖。
- 最终候选运行：
  - `scripts/dev_check.sh`
  - `scripts/full_gate.sh`
  - 三平台 CI 构建。

### 手工验收

1. 干净文档，点击红色关闭按钮：进程退出，Dock 图标消失。
2. 编辑文档后点击关闭：出现保存/丢弃/取消确认。
3. 取消：窗口保留，应用继续运行。
4. 保存或丢弃并解决全部脏文档：应用退出，Dock 图标消失。
5. 多脏文档混合保存/丢弃；中途取消 Save As：窗口保留，且不会残留更新重启。
6. Settings 窗口打开时关闭主窗口：应用退出，Settings 一并关闭（已接受语义）。
7. 有更新已准备时执行重启：先解决脏文档，再安装更新，不被提前退出打断。
8. 非 macOS：确认同一窗口关闭回调在 Windows/Linux 也退出，或明确记录本次只验收 macOS。

## 风险与取舍

- `cx.quit()` 在 `windowShouldClose` 回调返回前被调用，但 `Platform::quit` 自身异步派发，不会同步重入 `terminate`；直接调用与 `cx.defer` 等价，采用直接调用并注明这一点。
- 关闭主窗口会连带关闭 Settings 辅助窗口；这是“主窗口关闭即退出”的已接受语义。
- Cmd+W 当前没有绑定；本次只改变窗口关闭回调覆盖的路径，不新增快捷键。
- 若未来实现托盘/后台驻留，需要在 main 回调层增加偏好分支，并重新引入 `on_reopen`；本次明确不做。
- GPUI 测试平台的 `quit()` 是空实现，自动测试无法观察“确实请求退出”；该点必须由手工验收覆盖，报告中不得声称自动验证。

## 审查记录

### 第一轮（独立 reviewer）

结论：方向正确，无阻断；指出：

1. `restart_for_update` 的真实风险不是“提前 quit 打断安装”，而是安装失败后已排队的 quit 会终止进程，错误提示看不到。→ 已改为 main 回调层退出，`restart_for_update` 不受影响。
2. `pending_restart` 在 Save As 取消和安装失败后未清理。→ 已加入计划 §3。
3. GPUI 测试平台 quit 为空实现，重启门控无法自动观测；`restart_for_update` 也没有现成测试。→ 已加入 §测试与验收，明确手工验收。
4. Settings 窗口场景与 `app_state_test.rs` 过时注释。→ 已明确语义并加入清理项。

### 第二轮

结论：**无未解决实质异议**。非阻断修正：

1. `App::defer` 在当次 update 的 effect flush 中同步执行，并不延后到 `windowShouldClose` 返回后；
   真正避免重入的是 `Platform::quit` 自身异步。计划改为直接 `cx.quit()` 并修正注释。
2. 现有 `handle_modal_cancel`/`handle_modal_save`/`handle_modal_discard` UI 测试不存在；
   取消关闭由手工验收覆盖，必要时补窗口级测试。计划已更正。

审查版本确认该修订可执行，未发现新的具体失败场景。

## 实施结果（2026-11-09）

- `main.rs` 窗口关闭回调：允许关闭时 `cx.quit()`。
- `after_dirty_doc_resolved`：无 `pending_restart` 时 `cx.quit()`，不再 `remove_window()`。
- `prompt_save_as_and_close` 的 Save As 取消、`install_prepared_update` 的无 target/安装失败分支均清理 `pending_restart`。
- `app_state_test.rs` 过时注释已更新。
- `scripts/dev_check.sh`：374 passed；`scripts/full_gate.sh`：通过。
- macOS 手工验收：
  - 已保存文件点击红色关闭：进程退出。
  - 脏文件点击关闭：应用停留并显示确认；Esc 取消后窗口保留。
  - 脏文件丢弃（D）后进程退出，文件内容未写入。
  - 欢迎页首次关闭：不再弹保存确认，第一次点击即退出。
  - 测试期间临时关闭 auto_save，结束后已恢复用户配置。

### 补充根因（欢迎页首次关闭）

欢迎页由 `Document::new_scratch()` + `update_buffer` 生成，`last_saved_content` 为空，
导致初次显示就被标记为 `Modified`。同时 `next_dirty_doc_key` 还把“非空 scratch”
无条件视为脏。两处都会让首次关闭进入保存确认，而不是退出。

修复：
- 新增 `Document::new_scratch_with_text`，让生成的欢迎内容初始为 `Clean`；
- 删除 `next_dirty_doc_key` 中重复的“非空 scratch 即脏”条件，统一以
  `DocumentStatus` 判断；编辑过的草稿仍会变成 `Modified`。
