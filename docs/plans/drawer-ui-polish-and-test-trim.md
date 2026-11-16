# Drawer 搜索、文件夹与文件行 UI 优化及小批测试清理

> 当前代理约定（2026-11-16）：herdr-triad 已弃用；由当前代理自行审查，不向其他 pane 派任务。本文跨窗格与独立代理审查要求仅作历史记录，现行规则见 [代理工作约定](../agent-workflow.md)。

版本：v1（用户于 2026-11-05 明确取消既有审查流程，授权直接开发；记为“用户覆盖”，不写成审查员无异议）  
基线：`9991a31556a97918302486722268c2d35691d073`  
用户决策：2026-11-05 已确认 macOS 克制紧凑方向、调整文件行，并同意有证据地删除 9 项低价值测试。

## 目标与边界

针对用户截图中搜索框输入后拥挤、文件夹层级不清的问题，只调整 Drawer 呈现与已确认的低价值测试，不改变搜索、中文 IME、折叠、新建、选择、关闭或重名消歧行为。

生产代码只改 `crates/glidemark_app/src/views/drawer.rs`。复用现有 `ThemeColors` 与 `icons/chevron-right.svg`；不新增依赖、资产、状态字段、通用组件、测试框架或 ADR。`note — ZGWS` 的 Workspace Root 重名消歧规则保持不变。

## UI 方案

### 搜索槽

- 保持现有 28 逻辑像素高度；截图为 2×，不按物理截图像素误判控件高度。
- 容器左右内边距由 8px 增至 10px，搜索图标与输入文字间距由 6px 增至 8px；输入字号固定 13px，清除按钮保留足够命中区域。
- 用户后续确认：搜索框底色与 Drawer 的 `theme.bg` 完全一致，去掉静态边框和独立色块；仅悬停时复用 `theme.hover_bg()`，现有光标继续提供聚焦反馈，不添加阴影或新聚焦状态。
- 光标宽度收窄并缩短上下两端；只改绘制几何，不改 `EntityInputHandler`、选区、IME 或键盘处理。
- 占位文字由硬编码黑色透明度改为继承文字色的透明度，避免暗色 Palette 对比度错误；输入正文保持 `text_primary`，辅助图标保持 `text_muted`。
- 清除按钮悬停色改用主题前景色透明度，不再使用在亮色主题中近乎不可见的白色透明层。

### 文件夹与 Workspace Root

- 根目录和子目录均增加 10px 左右的 `chevron-right.svg`，展开时通过 GPUI `Transformation::rotate` 旋转 90°；不改原有整行点击折叠行为。
- 保留 open/closed 文件夹 SVG。根目录使用 13px Semibold，子目录使用 13px Medium；图标使用 `text_muted`，名称使用 `text_primary`，悬停沿用 `hover_bg()`。
- 保留现有行高、右键移除根保护、hover 新建文件按钮和路径 tooltip；不添加数量徽章、彩色文件夹或额外卡片。

### 文件行

- 保留 44px 双行结构、13px 文件名与 11px 日期；日期仍来自缓存的 `node.mtime`。
- 调整文件行左侧缩进，使文件名与文件夹层级有明确对齐关系，日期与文件名共用同一文本起点。
- 选中态继续复用 `selection_bg()` / `selection_text()`，通过间距、圆角和文字层级减轻当前大块选中区域的笨重感；不引入新的强调色或状态。

## 小批测试清理（独立测试提交）

只删除下列已逐项确认的低价值检查，共 9 项；不按年龄、数量或目标比例删测，不使用 `ignore`、放宽断言或缩小输入。

1. 删除整个 `crates/glidemark_app/tests/drawer_polish_test.rs` 的 5 项：全部仅用 `include_str!` 和源码字符串匹配约束旧样式、旧行高或禁止旧色值，没有经过生产 UI 行为入口，且直接阻碍已批准的视觉调整。
2. 从 `sidebar_two_tier_and_header_weight_test.rs` 删除 2 项：
   - `test_sidebar_two_tier_typography_and_arrow_removal`：锁定旧字重与字符箭头实现；本次使用 SVG 箭头，旧视觉契约不再成立。
   - `test_sidebar_two_line_file_items_and_single_line_folders`：与已删除的 Drawer 样式字符串检查重复，只锁定 44/28px 和字号字符串。
3. 从 `drawer_search_test.rs` 删除 2 项：
   - `test_drawer_render_contract`：只检查源码 token、字符串顺序和 emoji 是否存在，不能证明搜索框固定、输入、清除或状态提示的实际行为。
   - `test_drawer_search_params_from_state`：测试自行构造 `DrawerSearchParams`，再断言自己写入的字段；没有生产接线或行为判别力。

明确保留：中文 IME、光标/选区、搜索过滤与恢复、后台搜索和陈旧结果、文件夹展开状态、文件创建/关闭、资产真实加载，以及有有效算法或生产入口价值的其他检查。本批不处理全仓其他候选，不声称 9 项删除会显著提速；减少一个集成测试二进制及脆弱维护成本是已确认收益，实际耗时如实测量。

## 实施与验证节奏

1. UI 单独提交：只改 `drawer.rs`。
2. 开发中只运行：
   - `cargo test -p glidemark_app --test drawer_search_test --test sidebar_two_tier_and_header_weight_test --test asset_registration_test`
   - 必要时按名称运行 IME/选区检查，不在每次样式微调后运行全量 300+ 测试。
3. 启动真实应用检查默认暖石亮色：空搜索、中文输入、长查询、清除、文件夹展开/收起、文件选中与日期对齐；再抽查一个暗色 Palette。保存实际截图证据，不用自动像素测试替代人工检查。
4. 测试清理单独提交，逐项对应上述失效证据；提交前跑上述相关目标。
5. 最终候选按项目既定交付门槛运行 `cargo test --workspace`、AGENTS.md 要求的 feature 专项及 Phase H 21 项串行回归，分报 passed/failed/ignored 和重叠；随后运行 `./scripts/bundle_macos.sh`，检查严格签名与二进制时间。有限的中途验证不冒称完整回归。

## 验收

- 输入中文后，搜索图标、正文、光标和清除按钮之间有清晰、稳定的水平与垂直留白，无裁切或 IME 回归。
- 文件夹展开状态一眼可辨，根/子目录层级明确；文件行和日期对齐，选中态仍清晰但不过重。
- 搜索、折叠、新建、选择、关闭、右键移除根及重名消歧行为不变。
- 只删除批准的 9 项低价值测试；功能保护不减少，最终交付门槛不被削弱。


## v2 追加（2026-11-05 第二轮样式反馈）

用户第二轮截图指出三项，已按选择实施：

1. **头部与红绿灯错位**：真因是画布头部 46px（图标中心 23px）而红绿灯中心 18.75px；侧栏展开时用 38px 侧栏头部才接近对齐。改为画布头部 46px → 38px，两个头部共用 19px 中心；实测红绿灯 18.75px、图标 19.0px，差 0.25px。未移动系统红绿灯位置。
2. **标题颜色突兀**：编辑器单独使用 `HeadingPrefix`（暖锈色 `syntax_heading`）与 `HeadingText`（`text_primary` 加粗）。`syntax_heading` 默认暖石明/暗由 `#0969da`/`#58a6ff` 改为 `#a4502c`/`#d98a5f`（对比度 5.07:1 / 6.01:1），其余 7 套配色保留各自方案。
3. **保存按钮标红且无用**：移除顶栏保存按钮（自动保存与 `Cmd+S` 保留），删除 `topbar_save_button_color` 与 `render_save_icon`。

### 跨平台最小地基

`crates/glidemark_app/src/platform.rs` 集中 `UI_FONT`/`MONO_FONT`（macOS/Windows/Linux 各自字体栈）、`LEADING_WINDOW_CONTROL_INSET`、`TRAILING_WINDOW_CONTROL_INSET`；替换 25 处硬编码 macOS 字体名与头部内联常量。`cache_accounting_test` 改为按目标平台使用 `malloc_size` / `malloc_usable_size` / `_msize`。

仍未做（后续适配时处理）：macOS 打包脚本 `scripts/bundle_macos.sh`、工具提示与菜单里的 `⌘` 字面量（i18n 20 处）、Windows/Linux 窗口控制区实测、gpui 目标平台构建验证。


## v3 追加（2026-11-05 第三轮样式反馈，参考图 2 配色）

用户提供第二张参考图（暖色阅读器），要求减少全局蓝色、弱化标题标记、代码块加语言标签、状态栏改为字数。已实施：

1. **标题标记弱化**：编辑器 `HeadingPrefix` 改为 `text_muted` + 常规字重；`HeadingText` 保持 `text_primary` 加粗。预览标题本就为深墨加粗，与参考图一致。
2. **全局蓝色清理**：默认暖石主题 `syntax_link` / `syntax_bullet` 由 GitHub 蓝改为暖棕/暖赭（明 `#8A5A2B`/`#96603C`，暗 `#C89A6B`/`#C08A5E`，对比度 5.33/4.73/6.43/5.46）。任务复选框复用 `syntax_bullet`，随之去掉系统蓝。搜索框光标与选区不再用 `gpui::blue()` / `rgba(0x3311ff30)`，改为由主题文字色推导。其余 7 套配色保留自身蓝色身份，不做改动。
3. **代码块语言标签**：预览代码块右下角以 10px 等宽、`text_muted` 75% 显示围栏语言（如 `rust`），`text`/空语言不显示；不影响选区与复制。
4. **状态栏**：去掉未保存红点，去掉列号，显示 `行 N · M 字`；新增 `I18nStrings.status_chars` 覆盖 10 种语言，原 `status_col` 移除。
5. **删除死 token**：`ThemeColors.syntax_heading` 生产端已无读取方，连同 16 处配色值与对应断言一并删除。

### 滚动问题（未在本轮修复）

用户报告“滑到底继续滑动会弹回上方”“长文档滚动条不跟手”。已做两项实证，均**未能复现**：

- 400 块普通文档、纯滚轮事件 80 次：位置单调递增，到 `item=382` 后稳定停止，不回落。
- 600 块含 120 张缺失图片与 120 个 SQL 代码块的文档、260 次滚轮：`JUMPS=[]`，末位 `item=585` 稳定，滑块 `y` 单调且停在轨道末端。

真实应用自动化受阻：`screencapture` 可用，但 `osascript keystroke` 与 CGEvent 点击/按键无法注入（无辅助访问权限），无法在用户的 6976 行长文档上复现。修复需要具体复现条件（Split 还是 Preview、触控板惯性还是鼠标滚轮、是否发生在图片加载后），否则只能猜测。


## v4 追加（2026-11-06 滚动同步修复与语言标签调整）

用户补充了精确复现条件：**分屏模式**下滚动**左侧编辑器**到底部后继续滚动，预览滚动条乱跳、闪屏；并要求滚动平滑“跟手”、语言标签稍大但弱对比。

### 根因与修复（`workspace.rs`）

1. **预览跳回顶部**：`handle_editor_scroll_wheel` 用 `(scroll_y / row_h)` 求可见行下标，越界时 `visual_lines.get(idx)` 返回 `None`，随后 `unwrap_or(0)` 把 `phys_line` 变成第 1 行，预览被拉回第一块。滚轮事件在绘制前触发，而此时编辑器偏移可能已越过末端（gpui `uniform_list.rs:380` 只在绘制阶段把超界偏移夹回，说明超界偏移在事件阶段确实存在），于是每次滚轮都“跳到顶部再夹回”，表现为乱跳与闪屏。修复：下标 `min(len-1)`。
2. **大块内不跟手**：编辑器→预览的块内偏移被硬编码为 `fraction * 100px`，与真实块高无关，长段落/代码块里预览几乎不动。修复：`calculate_subline_scroll_offset` 改为返回 0..1 比例并重命名 `calculate_subline_scroll_fraction`，调用处乘以测得的块高。
3. **预览→编辑器方向丢进度**：`sync_preview_scroll` 用 `preview_to_source_line(top_block, 0.0)` 丢掉块内进度，滚轮在长块内不会带动编辑器。修复：按 `offset_in_item / block_height` 传入真实进度。
4. **闪屏另一来源**：语法需求检查在没有缺失块时也调用 `invalidate_preview_list()`，每次滚轮都 `splice` 整个虚拟列表并重置项高。修复：该分支改为仅 `cx.notify()`（语法 spans 在渲染时查缓存，布局未变）。

### 语言标签

`text_size` 10 → 11px，`text_muted` 透明度 0.75 → 0.6，位置仍在代码块右下角。

### 证据与边界

新增 `crates/glidemark_app/tests/split_scroll_sync_test.rs`（2 项，约 14 秒）：

- `editor_overscroll_never_sends_preview_back_to_start`：分屏、400 次滚轮把编辑器推到底部之后，预览项索引永不回退且停在文档末 10% 区间。
- `in_block_offset_uses_measured_block_height`：块内偏移必须落在测得块高内，且不再固定为 25 的倍数。变异回 `fraction * 100` 后该项 FAILED（实测偏移 50/100 越过 44/96px 块高并命中 25 倍数）。

**未覆盖**：headless 测试里 `uniform_list` 会把越界偏移夹回，因此第 1 项修复在测试中不可达，仅由上方代码路径推断；真实触控板超界仍未在真机自动复现（输入注入无辅助访问权限）。分屏滚轮实测仍待用户确认。


## v5 追加（2026-11-06 图片巨型块、拖动跟手与出界卡住）

用户反馈：分屏滚到底仍会“调回一下”；图片多的文件拖动滚动条不跟手；拖出窗口后卡住。

### 已修（`MAX_IMAGE_DISPLAY_HEIGHT`）

**根因**：内联图片只按 `aspect_ratio` + `max_w(原图像素宽)` 布局，没有任何高度上限。实测 400×1600 的竖图在 1042px 视口里生成 **1616px** 的单个块，一屏放不下一个块——既留出大片空白，也让滚动条的“每块一步”映射变得极不均匀（图片文件不跟手、纯文本文件正常）。

**修复**：按 `MAX_IMAGE_DISPLAY_HEIGHT = 400pt` 反推最大宽度 `min(400 × ratio, 原图宽)`，靠缩小宽度而不是 letterbox 来限高。同一张图块高 1616px → 414px。

判别证据：`tests/preview_image_sizing_test.rs` 用 `image` crate 现场生成 400×1600 / 1200×400 两张 PNG，断言最高图片块 ≤ 440px；把 `max_display_width` 改回 `image.width` 后该项 FAILED（实测 1616px）。

### 已修（拖出窗口卡住）

拖动结束只依赖窗口内 mouse-up。若指针移出窗口后松开，拖动态会继续保留，下次移回时光标位置被当成拖拽位置。现在根级 `on_mouse_move` 里检查 `event.pressed_button`：为 `None` 说明按钮已在别处松开，立即结束拖拽。

### 尝试并否决（像素映射）

原想改用 gpui 的 `set_offset_from_scrollbar` / `max_offset_for_scrollbar` 做像素级映射，以彻底解决变高块的拖动比例问题。实测否决：`max_offset_for_scrollbar` 只统计**已测量**项高（文档 1891px 可滚动时它给出 631.5），拖动到“底”实际只到文档 1/3，`phase_j_scrollbar_test::real_drag_reaches_end...` 立刻红。已回退，保留按块索引+实测块高的原映射。

### 仍未复现

“滚到底渲染界面调回一下”：分屏 220 次滚轮（含图片与代码块）到头后，`preview_item/item_offset` 完全稳定，`OSCILLATIONS=0`，headless 无法复现。真机输入注入无辅助访问权限，且本轮结束时显示器已休眠，截图取证失败。

## 独立规划审查

用户于 2026-11-05 明确指示“以前的模式不用了，直接开发即可”，覆盖原审查关口；本计划不宣称已取得独立审查签字。
