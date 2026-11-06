# 内存优化、界面微动效与 Windows/Linux 适配

状态：草稿 v4（已按三轮独立审查修订，待复审）。用户于 2026-11-06 确认：

1. **内存**：稳健目标——1.5MB 文档 163.4MB 目标 ≤110MB，空闲 107.2MB 目标 ≤75MB；测量驱动，不为指标牺牲手感。
2. **微动效**：小范围反馈动效 + “减少动态效果”开关；不做整页导航过渡。
3. **平台**：Linux 与 Windows 两轨并行；Linux 构建/打包，Windows 可行性 spike。
4. **本轮**：交付内存计划与平台计划（经审查）；实际实现微动效与减少动效开关；内存/平台生产代码另开阶段。

## 0. 首轮审查回应

| 审查项 | 处理 |
| :--- | :--- |
| O1 保存按钮/圆点不存在 | 不新增保存按钮；在**画布标题右侧新增 6px 未保存圆点**（新 UI 元素，dirty 显示、clean 隐藏），脉冲只作用于该圆点；“主按钮”按压限定为导出/设置两个现有按钮 |
| O2 `with_animation` 不会自动重播 | 所有需要重复播放的动画使用**状态化 keyed id**：模式指示 `format!("mode_indicator_{tab_id}")`（仅激活标签渲染下划线）、dirty 脉冲 `format!("dirty_pulse_{generation}")`、复制对勾 `format!("copy_check_{code_hash}_{occurrence_line}_{instant:?}")`、按压 `format!("press_{button_id}_{generation}")`；view 增加 generation 计数器；测试连续两次切换/两次 dirty/两次按压 |
| O3 M5 事实错误 | 更正为：lightbox 图缓存容量 **1**（仅 SVG 路径，`lightbox.rs:60`）+ `cx.drop_image` 释放 GPU 纹理 + `local_images` **64 MiB** 图片缓存；不再引用 ADR 0022 的“8 张”作为现状 |
| O4 目标与候选量级不闭合 | 增加 stop/go：先测 M1/M4；若可寻址项合计 <15 MiB，停止该路线并报告修订目标，不把未达 ≤110/≤75 包装成完成；明确 `bench_memory.sh` 是“8 次采样峰值 RSS”，不是 Physical Footprint |
| O5 feed 缺 Intel macOS | `platforms` 增加 `macos-aarch64`、`macos-x86_64`（可发布 universal），并定义旧顶层字段 fallback；增加兼容性测试 |
| O6 Linux portal 运行时 | 运行时依赖加入 `xdg-desktop-portal`、`xdg-desktop-portal-gtk`、`libvulkan1`、`mesa-vulkan-drivers`；Xvfb 不作为文件对话框验收环境，需真实桌面会话；portal 缺失时文档化/降级提示 |
| O7 macOS 清单不完整 | 补齐 activation observer、`malloc_good_size`、`profile_fps.sh`、`adaptive_header_padding`/`is_macos`；修正 README 只承诺 macOS 的事实 |
| O8 reduce_motion 缺 i18n | 新增 `reduce_motion_label`、`reduce_motion_desc`，补齐 10 个 locale，并加配置/设置测试 |
| O9 横幅退出动画缺状态 | 新增 `export_notice_closing` / `update_banner_closing` + 120ms timer；先保留元素播放退出，再置 None；generation 防止旧 timer 清掉新横幅 |
| O10 验收不可证伪 | 定义 p95 帧时间 ≤16.7ms、最差 ≤33ms（参照机，3s 交互 trace）；Windows spike 给出 pass/fail checklist |
| O11 事实/文案 | 改为“div 无 CSS transform；SVG `Transformation` 存在但本轮不用”；reduce=true 时**不包** `with_animation`（避免 duration 0）；补 Vulkan/portal；Windows `unimplemented!()` 纳入 go/no-go |
| O12 范围 | 不新增保存按钮；dirty 指示为小圆点；内存目标表述为目标而非承诺 |
| O1（v2 轮）keyed id 不唯一 | 统一规则：`mode_indicator_{tab_id}`（仅激活标签渲染）、`press_{button_id}_{generation}`、`copy_check_{code_hash}_{occurrence_line}_{instant:?}`、`dirty_pulse_{generation}`（唯一圆点）；补“同帧多元素”测试 |
| O2（v2 轮）帧时间验收口径 | 新增可复现的帧时间探针（env 开关，记录帧间隔）；参照机刷新率决定阈值：120Hz p95 ≤8.3ms/最差 ≤16.7ms，60Hz p95 ≤16.7ms/最差 ≤33ms；统一内存/动效引用同一方法 |
| O3（v2 轮）stop/go 区间 | A=可寻址收益：<15MiB 停止；15–40MiB 实现 top 候选并允许“未达结论”结束，不追加高成本路线；≥40MiB 进入实现 |
| O4（v2 轮）细节 | dirty 圆点仅表示**当前活动文档**；Vulkan 归入运行时/AppImage；p95 统一引用 §4.1 |
| O1（v3 轮）§2.5 口径矛盾 | 删除 `profile_fps.sh` 的 p95 判据；统一使用 `ZMARK_FRAME_PROBE` + `analyze_frame_probe.py` 与 §2.4 的 120/60Hz 阈值；profile_fps 仅作 CPU/滚动辅助 |
| O2（v3 轮）`on_next_frame` 一次性 | 探针每次回调记录后**重新注册** `on_next_frame`；3s 到期停止；样本先存内存、结束后一次落盘；自测要求 3s 至少采集 30 帧（防只采一帧） |
| O3（v3 轮）复制 id 对重复代码不唯一 | `copy_check_{code_hash}_{occurrence_line}_{instant:?}`（使用 ParsedCodeBlock 的 fence_open_line / block 位置）；补重复代码块测试 |
| O4（v3 轮）细节 | §2.5 主门禁改为 p95/worst；stop/go A 固定同构建/同文档/同窗口；AppImage 冒烟步骤安装 Vulkan/portal；feed v2 增加“旧字段+platforms 并存”兼容测试 |

## 1. 内存

### 1.1 基线（2026-11-06，ZMark 0.2.0，macOS release）

`./scripts/bench_memory.sh`：8 次采样峰值 RSS，6s settle，非 Physical Footprint。

| 场景 | 当前 | 目标 |
| :--- | ---: | ---: |
| 默认示例 | **107.2 MB** | ≤75 MB（目标） |
| 1.5MB / 16,546 标题 | **163.4 MB** | ≤110 MB（目标） |

旧 README 的 85/136–148MB 早于 v0.2.0，且口径/波动不同；本轮以新实测为准。

### 1.2 候选与已知事实

| 编号 | 位置 | 事实 | 可能收益/代价 |
| :--- | :--- | :--- | :--- |
| M1 | `views/editor.rs` `DocumentLayoutCache` | 每物理行保存 `String` 文本副本 + `Vec<LineVisualSpan>` + 宽度（`editor.rs:400-418`）；`VisualLine` 另存 Arc 向量 | 视口窗口 + 字节预算可降常驻；未测占比，需先量化；可能增加滚动时 shaping |
| M2 | `syntax.rs` | 语法缓存固定 4 MiB | 普通文档降到 2 MiB 最多省 2 MiB |
| M3 | 撤销栈 | 总字节上限 8 MiB | 降到 4 MiB 最多省 4 MiB；牺牲撤销深度 |
| M4 | ADR 0022 观察 | 大图表场景曾出现 `Malloc Large (empty)` 约 146 MB 未归还 OS | 大文档关闭/解析后 `malloc_zone_pressure_relief`（macOS）/`malloc_trim(0)`（glibc）可能降低空闲 RSS；当前占比未测 |
| M5 | 图表/图片 | lightbox 图缓存容量 1（SVG 路径）+ `cx.drop_image` 释放纹理；`local_images` 64 MiB | 核对释放时机与 `IOAccelerator` 实际变化 |
| M6 | GPUI 字体/atlas | 框架内部 | 仅测量，不承诺 |

### 1.3 方法、顺序与 stop/go

1. 基线：`bench_memory.sh` + `vmmap`（Malloc Large/Small、IOAccelerator、TEXT）+ `heap` + memory probe。
2. 先量化 M4（空闲堆）与 M1（编辑器缓存）；再决定 M2/M3/M5。
3. **Stop/go**（A = 同一构建、同一文档、同一采样窗口下测量到的 M1+M4+M2+M3+M5 可寻址收益合计）：
   - A < 15 MiB：停止该路线，报告真实构成与修订目标，不宣称目标达成；
   - 15 ≤ A < 40 MiB：实现占比最高的候选并复测；若仍低于目标，以“未达结论”结束，不追加高成本路线；
   - A ≥ 40 MiB：进入实现并冲击目标。
   （M2+M3 合计最多约 6 MiB，单独不足以支撑 53/32 MiB 的目标，必须依赖测量结果。）
4. 每项独立提交；行为/交互不变（除 M3 的撤销深度需明确报告）。
5. 若最终只做到部分收益，报告“实际值 + 剩余构成 + 未达目标原因”，状态不得写完成。

### 1.4 验收

- 达成目标或给出可证伪的未达结论（实际值、剩余构成、是否继续）。
- 全量/观测/Phase H 全绿；新增缓存驱逐/复用测试。
- 大文档滚动/输入不回退：按 §2.4 帧探针口径（120Hz 或 60Hz 对应阈值）与基线对照。
- 明确列出撤销深度/语法高亮预算等取舍。

### 1.5 风险

- M1 窗口化 shaping 可能卡顿；保留预取窗口，先测。
- M4 仅 macOS 直接可用；Linux 用 `malloc_trim`，Windows 无等价，需分别验证。
- RSS 峰值与 Physical Footprint 口径不同，报告分开。

## 2. 界面微动效（本轮实现）

### 2.1 原则

- GPUI 0.2.2 的 `div` 无 CSS transform；本轮只用 opacity 与小幅高度/布局变化。SVG `Transformation` 存在但本轮不使用。
- 120–180ms，`ease_out`/`ease_in_out`；只做出现/消失/一次性反馈；不持续循环。
- `reduce_motion=true` 时**不包** `with_animation`，直接渲染终态（避免 duration 0 的除零问题）。
- 可重复播放的动画必须使用状态化 id（见 §0 O2），不能依赖固定 id 的常驻元素自动重播。

### 2.2 动效清单与目标元素

1. **顶部横幅出现/退出**：导出成功、更新就绪两条横幅。出现：高度 0→32/36 + opacity 0→1（160ms）；退出：opacity/高度→0（120ms）后卸载。新增 `export_notice_closing` / `update_banner_closing` 状态与 120ms timer；generation 防止旧 timer 清掉新横幅。错误横幅保持即时。
2. **模式切换指示**：在三个模式标签内增加 2px 下划线；id = `format!("mode_indicator_{tab_id}")`（每个标签唯一，仅激活时挂载），每次切换重新挂载并 140ms 淡入。不做内容面板交叉淡化。
3. **未保存圆点**：在画布标题右侧新增 6px 圆点，仅表示**当前活动文档**的 dirty（dirty 显示、clean 隐藏），不新增保存按钮；`format!("dirty_pulse_{generation}")`，clean→dirty 时 200ms opacity 脉冲，dirty 期间静态；圆点只表示当前活动文档。
4. **复制对勾**：代码块复制按钮的对勾图标 120ms 淡入；id = `format!("copy_check_{code_hash}_{occurrence_line}_{instant:?}")`（`occurrence_line` 取 ParsedCodeBlock 的 `fence_open_line`，重复内容也唯一；`Instant` Debug 对同一复制时刻稳定）。
5. **导出/设置按钮按压**：90ms opacity 回弹；id = `format!("press_{button_id}_{generation}")`，每次 mouse down 增加 generation；导出/设置按钮各自唯一。仅这两个按钮；保存仍走 Cmd+S，不加按钮。

### 2.3 减少动态效果

- `Preferences.reduce_motion: bool`（serde default false）；设置 → 外观 开关。
- 新增 i18n：`reduce_motion_label`、`reduce_motion_desc`，补齐全部 10 个 locale。
- 所有动画入口统一 `motion_delta(reduce, delta)`；reduce=true 时不包动画元素。
- 测试：默认反序列化 false；true 往返；设置切换持久化；helper 终态；i18n 完整性由编译保证。

### 2.4 帧时间测量（统一口径）

- 新增调试用帧探针：`ZMARK_FRAME_PROBE=1` 时注册 `on_next_frame`；**每次回调记录 `Instant` 后重新注册**，直到 3s 窗口结束；样本先存内存，结束后一次写文件（避免 UI 线程重 I/O）；`scripts/analyze_frame_probe.py` 计算 p95/最差/刷新率。
- 自测：3s 内至少采集 30 帧，否则判为探针失败（防止只采一帧的假绿）。
- 参照机刷新率决定判据：120Hz → p95 ≤8.3ms、最差 ≤16.7ms；60Hz → p95 ≤16.7ms、最差 ≤33ms；报告实际刷新率与 hitch 数。
- `profile_fps.sh`（Instruments Time Profiler）只作为 CPU/滚动辅助证据，不作为帧时间 p95 判据。
- 内存与动效验收共用本节方法，不再各自定义阈值。

### 2.5 验证

- 现有测试全绿；新增配置/helper/元素存在性测试；连续两次模式切换、两次 dirty、两次复制/按压的 generation 断言。
- 主门禁：`ZMARK_FRAME_PROBE=1` + `analyze_frame_probe.py` 采集 3s 交互，按 §2.4 的 120Hz/60Hz 阈值判定；`profile_fps.sh` 仅作 CPU/滚动辅助，不参与 p95 通过条件。
- 人工：无闪烁/布局跳动作为补充观察，不作为主判据。
- 不影响滚动/输入/选区。

## 3. Windows / Linux 适配构建

### 3.1 现状事实

- GPUI 0.2.2 默认 feature 含 `wayland`、`x11`、`windows-manifest`；Linux/Windows 后端与文件对话框源码存在；README 实际只承诺 **macOS**，Linux/Windows 需实测。
- 未完成清单（补齐）：
  - updater（curl/ditto/codesign/xattr/lsregister、`.app` 换位）
  - `scripts/bundle_macos.sh`、`release.sh`、`profile_fps.sh`（xcrun）
  - 窗口标题栏/交通灯、`adaptive_header_padding`/`is_macos`、`⌘` 文案（20 处）
  - `install_window_activation_observer` 的 macOS 说明与行为
  - `local_images.rs` 的 unix `dev/ino`
  - `syntax.rs` 的 `malloc_good_size`（macOS）与各平台 heap 统计
  - `main.rs` 的 TitlebarOptions/traffic_light_position
- 无 CI；跨平台不能交叉编译，需原生 runner。

### 3.2 平台适配层（P0，macOS 行为不变）

`src/platform/{mod,macos,linux,windows}.rs`：字体栈/内边距、窗口选项/标题栏、reveal/open、快捷键文案、更新安装适配、减少动效查询（本轮用偏好）。

- 快捷键：macOS `cmd-*`，Linux/Windows `ctrl-*`（先验证 GPUI 对 `cmd` 的归一化；无归一化则显式绑定）。
- 窗口 chrome 遵循 ADR 0029；Linux/Windows 用系统标题栏或预留控件区，实测后定。
- 更新器：macOS 保持；Linux/Windows 先禁用并显示原因；自动安装后续阶段。

### 3.3 Linux 轨

- 编译依赖：`build-essential pkg-config clang cmake libssl-dev libfontconfig1-dev libfreetype6-dev libxkbcommon-dev libwayland-dev libx11-xcb-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev `（以首次编译错误为准补齐）。
- 运行时/AppImage：`libvulkan1`、`mesa-vulkan-drivers`、`xdg-desktop-portal`、`xdg-desktop-portal-gtk`（文件对话框 ashpd portal）；Xvfb 不作为对话框验收环境，需真实桌面会话；portal 缺失时给出明确提示/降级。
- 测试：`xvfb-run -a cargo test --workspace`；真实会话冒烟打开/保存/导出/分屏滚动/输入法。
- 打包：AppImage（依赖系统库或捆绑，文档化）+ `.deb`；`.desktop` + 图标；干净 Ubuntu 冒烟步骤需先安装 `libvulkan1`、`mesa-vulkan-drivers`、`xdg-desktop-portal`、`xdg-desktop-portal-gtk`。
- CI：`ubuntu-22.04` job：apt 依赖 → build → tests（Xvfb）→ AppImage/deb artifact。

### 3.4 Windows 轨（spike）

- `windows-latest` + MSVC；`cargo build -p zmark_app`。
- **Go/no-go checklist**：编译成功；窗口出现；打开/保存/导出；无 panic；文件对话框；Ctrl 快捷键；字体/窗口控件；若触发 GPUI 的 `unimplemented!()`（`hide_other_apps`/`unhide_other_apps`/`HasDisplayHandle`）则记 BLOCKED。
- 通过：zip 便携包冒烟；MSI/NSIS/更新器后续。失败：精确错误清单与缺口，不承诺安装包。

### 3.5 feed v2

```json
{
  "version": "0.3.0",
  "published_at": "…",
  "platforms": {
    "macos-aarch64": {"url": "...", "sha256": "...", "signature": "..."},
    "macos-x86_64":  {"url": "...", "sha256": "...", "signature": "..."},
    "linux-x86_64":  {"url": "...", "sha256": "...", "signature": "..."},
    "windows-x86_64":{"url": "...", "sha256": "...", "signature": "..."}
  }
}
```

- 每平台独立签名；载荷 `version\nplatform\nurl\nsha256\npublished_at`。
- 兼容：macOS 旧顶层字段保留一个版本；v2 App 优先 `platforms`，未知平台禁用并显示原因；Intel 读不到 `platforms` 时回退旧字段。
- 测试：旧 flat manifest 可读；v2 各平台选择正确；缺平台禁用；签名/哈希失败拒绝；同时含 top-level 旧字段与 `platforms` 时，旧 app 读顶层、v2 app 优先 `platforms`，两者解析一致。
- 资产命名 `ZMark-<version>-<platform>.<ext>`；发布脚本按平台构建并合并 manifest。

### 3.6 顺序与验收

1. P0 平台适配层 + 快捷键/窗口抽象（macOS 全量回归）。
2. P1a Linux 构建/测试/打包；P1b Windows spike（并行）。
3. P2 feed v2 + CI/release 布局。
4. P3 打磨。

- Linux：CI 通过；AppImage 干净 Ubuntu 启动；X11/Wayland + 真实桌面文件对话框/输入法冒烟；打开/保存/导出可用。
- Windows：go/no-go checklist 明确；通过则冒烟，失败则 BLOCKED + 错误清单。
- macOS：全量/观测/Phase H/打包/更新不受影响。

### 3.7 风险

- GPUI Windows 未承诺，可能不可用；spike 是硬门禁。
- Linux portal/Vulkan/字体差异；AppImage 依赖与体积。
- feed v2 是签名协议变更，需 App 与发布同步；旧版只读旧字段。
- CI 成本；Linux + macOS 优先，Windows spike 独立 job。

## 4. 本轮交付

- 实现 `Preferences.reduce_motion`、i18n、设置开关、四类微动效（+ 导出/设置按压），使用状态化 id 与退出状态。
- 交付本计划的内存/平台部分并完成独立审查；内存/平台生产代码不在本轮修改。
- 命令：`cargo test --workspace`、观测专项、Phase H、`./scripts/bundle_macos.sh`；`ZMARK_FRAME_PROBE=1` + `analyze_frame_probe.py` 采集 3s，报告参照机刷新率与 p95/worst；人工观察作为补充。

## 4.1 实施结果（2026-11-06）

- 已实现：`Preferences.reduce_motion` + 外观开关 + 10 locale i18n；顶部导出/更新横幅的进入/退出动画（closing 状态 + 120ms timer + generation）；模式切换下划线（`mode_indicator_{tab_id}`）；标题旁未保存圆点（`dirty_pulse_{generation}`，仅活动文档）；复制对勾淡入（`copy_check_{code_hash}_{occurrence_line}_{instant:?}`）；导出/设置按钮按压（`press_{button_id}_{generation}`）。
- 帧探针：`ZMARK_FRAME_PROBE` + `scripts/analyze_frame_probe.py`；本机参照屏 **120.1–120.2Hz**，动效运行 **p95 9.29–9.32ms、最差 48.89–54.27ms**；关闭动效（reduce_motion=true）同一交互 **p95 10.02ms、最差 54.70ms**。结论：动效未引入回退；绝对 120Hz 阈值（p95 ≤8.3 / 最差 ≤16.7ms）未达，原因是模式切换的内容重排（基线同样超限），属已知未达项，不包装为通过。
- 测试：`cargo nextest run --workspace` 361/361 通过；全量/观测/Phase H 见交付报告。
- 构建：`target/bundle/ZMark.app` 已重新构建并 Ad-Hoc 签名；因用户侧 ZMark 正在运行，`bundle_macos.sh` 按安全规则中止安装，`/Applications/ZMark.app` 仍为 0.2.0 旧构建；需退出 ZMark 后重跑脚本。
- 内存/平台：仅计划，未改生产代码；待后续阶段。

## 5. 独立审查记录

- 首轮（v1）：结论“有异议”，O1-O12 已写入 §0 并修订。
- 第二轮复审（v2）：仍“有异议”；O1-O4（keyed id 唯一性、帧时间口径、stop/go 区间、dirty/Vulkan/阈值细节）。v3 已逐条处理并写入 §0。
- 第三轮复审（v3）：仍“有异议”；O1-O4（§2.5 口径矛盾、on_next_frame 一次性、复制 id 重复内容、细节）。v4 已逐条处理并写入 §0。
- 第四轮复审（v4）：**结论“对该 revision 无未解决实质异议”**。审查版本 SHA-256 `5045898ec1cbc9a3684ed822dc73b78efafcf8fab5d0f21692f7f4a6ec30ad33`。非阻塞提醒：帧探针重新注册 `on_next_frame` 时需确保有下一帧（`request_animation_frame` 或等效驱动），30 帧自测会捕获。
