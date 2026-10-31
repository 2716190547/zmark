# ADR 0033: 现代化设置视窗、侧栏层级降噪、灯箱纸张画布与沉浸阅读排版

## 状态
提议 (Proposed) - 等待 Ticket D1、D2、D3 分步实施验收后转为 Accepted

## 上下文 (Context)
在 Phase C（视觉精进与 HiDPI 灯箱重采样）落地后，针对用户实际使用中的 4 处核心视觉与交互反馈进行了深度审视：
1. **流程图灯箱半透明底色发脏 (Image 0)**：当前灯箱采用 `rgba(0,0,0,0.45)` 浅暗蒙版，底层正文文字直接透过图表透射出来；且 Mermaid 图表无实体底色，半透明节点与深色背景及穿透文字多重叠加，产生脏污视觉。
2. **设置窗口设计语言断代脱节 (Image 1)**：设置窗口内部控件仍沿用早期的固定厚重边框（`border_1`）、高饱和度橙色标题框与传统分段按键，与主工作区已落地的无框暖石微胶囊（`warm_pill`）现代极简风格割裂。
3. **侧栏层级噪音、微型字体与无意义灰点 (Image 2)**：
   - 侧栏顶部常驻“工作区”分组标签，增加了无意义的视觉层级；
   - 树状列表在非活跃打开文档前强制渲染灰色实心小圆点（`·`），在无 Tab 架构下产生“未读/未保存/脏数据”的视觉困惑；
   - 侧栏字体仅 12px，与画布正文对比过小且拥挤。
4. **编辑器排版与交互体验痛点 (Image 3)**：
   - 侧栏条目 Hover 采用纯白 `#FFFFFF`，在暖石底色上对比刺眼突兀；
   - 顶部文档标题位置与下方编辑器代码文本未垂直对齐（存在行号槽位偏差）；
   - 编辑器硬编码 1.5x 行高视觉局促，虽 `Preferences` 已声明 `line_height: 1.6` 但未向编辑器传参，且设置界面缺少调节入口；
   - 编辑器缺乏基础的 Markdown 语法高亮。

## 决策 (Decisions)

### 1. 灯箱纸张画布与高纯度遮罩隔离 (Ticket D1)
- **纸张画布 (Paper Canvas)**：在 `lightbox.rs` 中的图表渲染容器内衬注入实体底色（浅色模式采用极简白纸/暖石面板，深色模式采用深石色），添加轻量圆角（`rounded_xl`）与柔和漫反射阴影（`shadow_lg`）。
- **纯净背景遮罩**：背景遮罩提升为高纯度深色蒙版（`rgba(0, 0, 0, 0.88)`），彻底阻断底层正文透光与视觉杂讯。

### 2. 侧栏层级平铺、无意义灰点清除与字体升级 (Ticket D1)
- **剔除“工作区”标签**：移除抽屉顶部的“工作区”大标题，多根 Roots 文件夹直接顶层平铺罗列。
- **消灭无意义灰点**：彻底移除 `is_open && !is_active` 下渲染的 5px 灰色实心圆点。
- **字体与行距层级提升**：
  - 侧栏节点字号从 12px 提升至 13px，行高增至 28px，增加舒适行内边距；
  - 文件夹与根目录字体略微加重，建立清晰视觉节奏。
- **Hover 态柔化**：摒弃 `#FFFFFF` 纯白高光，采用轻透暖石微胶囊浅色半透高亮（`warm_pill_hover`）。

### 3. 设置窗口现代无框重构与组件复用 (Ticket D2)
- **窗口原生装饰保留**：严格保留 macOS 原生窗口标题栏、拖拽手柄区与系统红绿灯按钮（遵循 ADR 0029，支持正常拖动与关闭视窗）。
- **内部控件“无框化”**：
  - “无框化”严格指向窗口内部控件：剥离左侧 Tab 项、模式按钮与卡片外层的生硬 `border_1` 描边；
  - 左侧导航全面复用与 Drawer/Header 一致的 `warm_pill` 无框微胶囊，选中态采用暖石胶囊高亮；
  - 外观模式切换由厚框按钮重构为无框平铺胶囊；调色板列表引入圆形色样（Color Swatch）与轻透卡片。
- **复用并激活 Preferences::line_height 配置**：
  - 复用 `glidemark_core::config::Preferences` 现有的 `pub line_height: f32`（默认 1.6，有效范围 `1.3..=1.7`，越界自动 fallback 回 1.6）；
  - 在 `SettingsTab::EditorSave` 中新增行高比率微胶囊调节组件（1.3x, 1.4x, 1.5x, 1.6x, 1.7x）；
  - 在 `glidemark_core::i18n::I18nStrings` 中补充 `line_height_label`，覆盖全部 10 种支持语言；
  - 修改后即时同步至主工作区并落盘持久化。

### 4. 编辑器视觉对齐、动态行高与语法分词着色 (Ticket D3)
- **标题槽位像素对齐**：调整顶部文档大标题左内边距，使其与编辑器代码文本第一列基准线垂直对齐（左侧对齐行号槽位 48px 边缘）。
- **编辑器接入动态行高**：
  - `render_editor_pane` 废弃硬编码的 `1.5`，接收 `line_height: f32`；
  - 单行高度动态计算为 `h(px(font_size * line_height))`，行内文本应用 `line_height(relative(line_height))`，保持光标与行号 1:1 匀称对齐。
- **轻量纯 Rust Markdown 语法分词着色**：
  - 在 `crates/glidemark_app/src/views/editor.rs` 中实现轻量单行 Markdown 分词渲染器：
    - 标题行（`#` 至 `######`）高亮为 accent 语义色；
    - 行内代码（反引号包裹）、链接（`[...]`）、加粗（`**...**`）、引用符号（`>`）赋予清晰层次色；
  - 仅在单行行内执行微秒级分词，零外部 C/JS/重型 AST 依赖，单帧耗时 <0.1ms。
- **Soft Wrap 架构边界与解耦**：
  - 明确架构约束：GPUI `uniform_list` 核心依赖每个虚拟行具备相同的物理像素高度；若在 `uniform_list` 中混入视觉软折行，将必然破坏 ADR 0024 确立的非重叠基线，导致行文本重叠与光标错位；
  - 本期 D3 聚焦于“对齐 + 动态行高 + 语法高亮”，严禁带病引入破坏 `uniform_list` 的半成品 Soft Wrap；
  - 软折行需求将作为独立架构演进课题，待后续设计非均高列表或自研折行布局缓冲引擎时独立立项。

## 任务拆解与推进节奏

- **Ticket D1 (侧栏降噪、排版升级与灯箱纸张画布)**：
  - 范围：`crates/glidemark_app/src/views/lightbox.rs`, `crates/glidemark_app/src/views/drawer.rs`, `crates/glidemark_app/src/theme.rs`
  - 独立测试套件：`tests/drawer_polish_test.rs`, `tests/lightbox_paper_canvas_test.rs`
- **Ticket D2 (设置窗口全量无框现代化重构与行高偏好接入)**：
  - 范围：`crates/glidemark_app/src/views/settings.rs`, `crates/glidemark_core/src/i18n.rs`, `crates/glidemark_app/src/views/workspace.rs`
  - 独立测试套件：`tests/settings_modern_ui_test.rs`
- **Ticket D3 (编辑器视觉对齐、动态行高与 Markdown 语法高亮)**：
  - 范围：`crates/glidemark_app/src/views/editor.rs`, `crates/glidemark_app/src/editor_engine.rs`, `crates/glidemark_app/src/views/workspace.rs`
  - 独立测试套件：`tests/editor_reading_enhancement_test.rs`

## 后果与影响 (Consequences)
- **正面收益**：
  - 灯箱图表阅读体验大幅飞跃，彻底告别文字穿透与杂色干扰；
  - 侧栏纯净度提升，无多余“工作区”层级与噪音小圆点，字号提升至 13px / 28px，舒适护眼；
  - 设置窗口与主界面设计语言实现 100% 统一，消除视觉割裂，保留原生窗口控制；
  - 偏好设置中的 `line_height` 真正生效，支持 10 种语言无缝切换；
  - 编辑器对齐美观，正式具备现代化 Markdown 语法着色体验。
- **架构约束与兼容**：
  - 语法分词在单行渲染内轻量进行，严格遵守单帧微秒级预算，杜绝复杂文档掉帧；
  - 严格保持偏好设置向前兼容，`line_height` 缺省或异常时优雅回退至 1.6。
