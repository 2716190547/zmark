# ADR 0032: 视觉层级精进、无感知全屏适配与 HiDPI 流程图重采样灯箱

## 状态
提议 (Proposed) - 等待 Ticket C1 与 C2 分步实施与终验后转为 Accepted

## 上下文 (Context)
在 Phase B 多文件夹联合工作区落地后，日常深度使用中发现以下 6 处直接影响使用体验的细节：
1. **全屏侧栏空隙**：macOS 全屏模式下红绿灯自动隐藏，但侧栏顶部依然硬编码 `px(76.)` 左边距，留下大片突兀的无效空白。
2. **侧栏草稿层级反直觉**：侧栏最上方常驻“草稿”大分组标签，即便未打开草稿也常驻灰色占位项，割裂了多文件夹工作区的直觉浏览体验。
3. **文档大标题视觉比重偏轻**：38px 头部内当前文档大标题字号仅 14px，视觉重心未能与右侧按钮水平中心稳重对齐。
4. **视图切换按钮过度强调**：源码/分屏/预览三档按钮外层包裹了厚重的分段边框容器，与右侧平铺的保存/导出/设置按钮在视觉节奏上产生割裂。
5. **流程图放大入口破坏严肃规范**：卡片右上角使用 `🔍` Emoji 与长文本（“点击放大查看流程图”），违背零 Emoji 严肃排版规范。
6. **灯箱放大位图模糊**：Mermaid 流程图在灯箱放大至 200%~500% 时有明显锯齿与失真，因原架构受限于 1000px 固定纹理栅格化尺寸。

经 Reviewer 架构初审，确认 C1（Header & Drawer 视觉重构）风险低、价值直接，可先行实施；C2（HiDPI 灯箱重采样）存在 4K 显存膨胀与 LRU 穿透风险，需在明确像素预算上限与独立释放策略的前提下分步实施。

## 决策 (Decisions)

### 1. 全屏动态自适应左边距 (Ticket C1)
- 在 `render_drawer` 与 `render_canvas_header` 传参中显式注入 `is_fullscreen: bool`（由 `window.is_fullscreen()` 获取）：
  - **普通窗口模式**：维持 `px(76.)`，避让 macOS 原生红绿灯按钮；
  - **全屏模式**：即时切换为 `px(12.)`（无需冗余动画），彻底消除顶部空隙。
- **全屏悬浮红绿灯取舍**：macOS 全屏下鼠标移至顶部滑出系统菜单时，红绿灯短暂悬浮覆盖左上角；此为 macOS 标准全屏软件（Safari/VS Code）通用行为，接受短暂悬浮覆盖的视觉取舍。

### 2. 侧栏草稿组织去中心化 (Ticket C1)
- **唯一归属判定**：
  - 彻底移除常驻的“草稿”一级大分组标题（`drafts_group`）与未打开时的空占位项（`draft_unopened`），相关死 key 标记废弃；
  - 当无未保存草稿时，侧栏零占用、零干扰，直觉呈现工作区文件夹树；
  - 当存在打开的未保存草稿（`DocKey::Scratch`）时，草稿作为独立单行排在工作区根目录树（Roots）之前，支持高亮微胶囊、未保存 dirty 提示、悬停 `[×]` 关闭确认。

### 3. 右侧大标题字号提升与严格垂直居中 (Ticket C1)
- 当前文档标题字号提升至 **15px**，保持 **Semibold** 字重；
- 显式声明行高与最大宽度截断（`truncate`），防止长文件名挤压右侧操作区；
- 在 38px 头部高度内严格通过 Flexbox 居中对齐，与右侧工具栏操作按钮处于绝对同一水平视觉中线。

### 4. 视图切换按钮彻底去边框平铺与暖石统一 (Ticket C1)
- 彻底移除视图切换按钮外层的 `border_1()` 分段边框容器与加深底色；
- 三个视图模式图标与右侧的保存、导出、设置按钮平铺统一对齐，点击热区维持 ≥ 32×32，具备 `cursor_pointer` 与 Tooltip；
- 选中项样式与左侧栏文档选中项保持一致，采用统一的高级暖石微胶囊底色（`theme.warm_pill_bg()` / `theme.warm_pill_text()`），非选中项使用标准中性弱化色。

### 5. 流程图放大入口纯 Icon 化 (Ticket C2)
- 彻底移除 `🔍` Emoji 与“点击放大查看流程图”文字；
- 改为流程图卡片右上角悬停显现的纯矢量微图标按钮（尺寸 ≥ 28×28，带 `cursor_pointer`），悬停呼出本地化 Tooltip（使用既有 `strings.mermaid_zoom_hint`，严禁中文硬编码）；
- 图标按钮点击事件调用 `e.stop_propagation()`，阻断冒泡到流程图卡片底层。

### 6. HiDPI 流程图灯箱独立重采样与显存保护 (Ticket C2b)
- **预览卡片预算不变**：正文图表继续严格执行 1000px 纹理预算（遵循 ADR 0022），绝不全局拉高默认栅格化尺寸；
- **SVG 矢量无损提取与重采样**：正文图表生成的 SVG 带有完整的 `viewBox` 矢量坐标。灯箱打开时，无须重新运行昂贵的 Mermaid 语法解析与布局，直接从现有矢量 SVG 提取几何比例，按需生成 HiDPI 变体；
- **16MP GPU 物理像素硬顶算法 (Pure Function)**：
  - 核心算法纯函数：`calculate_hidpi_dimensions(orig_w: f64, orig_h: f64, scale_factor: f32) -> (f64, f64)`；
  - 边界防守：`scale_factor` 若 `<= 0.0` 则安全回退为 `1.0`；若 `orig_w <= 0.0 || orig_h <= 0.0` 或解析失败，安全回退至标准尺寸 `(1000.0, 600.0)`；
  - 缩放档位确定：`scale_bucket = (scale_factor.clamp(1.0, 3.0) * 2.0).round() as u8`（例如 1.0x -> 2, 1.25x -> 3, 1.5x -> 3, 2.0x -> 4, 2.5x -> 5, 3.0x -> 6），消除同 key 不同分辨率抖动；
  - 设定最大 GPU 物理像素总量 `MAX_PHYSICAL_PIXELS = 16_000_000.0`（16MP），最大物理长边 `MAX_PHYSICAL_SIDE = 4000.0`；默认标准目标 8MP，4K/超大图按需升至 16MP 硬顶；
  - 面积预算计算：`area_ratio = (MAX_PHYSICAL_PIXELS / (orig_w * orig_h * (scale_factor as f64).powi(2))).sqrt()`；
  - 长边预算计算：`side_ratio = MAX_PHYSICAL_SIDE / (orig_w.max(orig_h) * scale_factor as f64)`；
  - 取更小缩放比 `final_ratio = area_ratio.min(side_ratio)`；逻辑尺寸向下取整，并显式断言 `physical_w * physical_h <= 16_000_000.0` 且长边 `<= 4000.0`；
- **SVG 根节点像素预算重写函数**：
  - 提供生产级纯函数 `apply_texture_pixel_budget(svg: &str, target_w: f64, target_h: f64) -> String`；
  - 严密支持 width="100%"、缺失 width/height、仅 viewBox 等三种异常输入；保留原有 viewBox 与全部矢量绘制指令，仅安全重写根节点宽高；
- **Arc<RenderImage> 持有与 drop_image 生命周期闭环 (严格显存控制)**：
  - `LightboxState` 显式持有 `hidpi_svg_path: Option<PathBuf>` 与 `hidpi_image: Option<Arc<RenderImage>>`；
  - 打开灯箱时：从独立文件缓存（`LIGHTBOX_DIAGRAM_CACHE`，单图 LRU 1）读取或生成 HiDPI SVG，通过显式解码生成 `Arc<RenderImage>`；
  - 渲染视图：`render_lightbox` 使用 `img(ImageSource::Render(handle.clone()))`；
  - 关闭与切换释放：在灯箱关闭（`LightboxState::close`）或切换当前文档时，显式调用 `cx.drop_image(handle, Some(window))`，立即从 GPU 显存回收 16MP 纹理；
  - 内存基线指标：灯箱打开与 5x 缩放时峰值内存控制在 ≤ 200MB，灯箱关闭后立刻回落至日常 ≤ 110MB 低内存基线；
- **5.0x 缩放基线**：在 `LightboxState` 最大 5.0x 缩放或 4K 全屏下，HiDPI 纹理在 16MP 预算内保持高锐利度；超出 16MP 预算的极端超长图由 GPU 平滑双线性插值承接，兼顾极清画质与 100% 显存安全。

## 分期与 Ticket 落地规划
- **Ticket C1 (Header & Drawer Visual Polish) [已交付]**：
  - 全屏 `is_fullscreen` 动态感知与 12px/76px 即时切换；
  - 侧栏常驻草稿分组与未打开占位行移除，活动草稿置顶单行收纳；
  - 大标题 15px Semibold 居中与防挤压截断；
  - 视图模式平铺去边框与暖石统一。
- **Ticket C2a (Mermaid Vector Icon Button) [已交付]**：
  - 流程图卡片右上角彻底移除 `🔍` Emoji 与文字，改为纯矢量微图标微胶囊按钮（28×28px，半透明暖石底色，悬停高亮）；
  - 内置纯矢量展开图标，绑定本地化 Tooltip，点击阻断冒泡；
  - 对应独立单测套件 `mermaid_icon_button_test.rs`。
- **Ticket C2b (HiDPI Lightbox Resampling) [规划中]**：
  - 灯箱按需 HiDPI 矢量重采样、16MP GPU 物理像素硬顶约束与 `drop_image` 释放机制；
  - 对应单元测试与显存安全断言。
- **Ticket C3 (Integration & Verification)**：
  - 全量 `cargo test --workspace` 回归验证；
  - 构建更新 `/Applications/GlideMark.app`；
  - 更新 ADR 状态为 Accepted，归档 GitHub 留痕 PR。

## 结果 (Consequences)
- 全屏沉浸式体验大幅提升，消除顶部无效空白；
- 侧栏结构更紧凑自然，不再被静态空草稿干扰；
- 顶部导航栏视觉节奏完全统一，大标题更有分量；
- 流程图在灯箱高倍缩放场景下实现高清水准，同时通过 16MP 硬顶杜绝显存爆炸隐患。
