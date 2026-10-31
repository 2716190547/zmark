# ADR 0034: 纯净中性调色体系、Apple 原生分组设置与均高切分软折行

## 状态
提议 (Proposed) - 等待 Ticket E1、E2、E3、E4 分步实施验收后转为 Accepted

## 上下文 (Context)
在 Phase D（设置微胶囊化与基础阅读排版）落地后，针对用户提交的 3 张视觉标注与参考设计（图 1 设置排版、图 2 现状痛点、图 3 妙言清爽排版）进行了系统性工程审视：
1. **颜色发脏与行内代码黄褐混杂 (图 2 & 图 3)**：当前默认主题存在黄褐色杂质（如 `code_bg` 为 `#EEECE6`），行内代码叠加 `accent`（橙红）后整体观感发脏；代码块标签与文本偏细发灰。参考图 3（妙言），亟需全面重构为纯净中性色。
2. **侧栏层级杂乱与误触风险 (图 2)**：侧栏字体大小未统一，三角标（`▸`/`▾`）与文件夹图标重复冗余；文件夹 Hover 默认展示删除按钮（`×`），极易误删工作区根目录。
3. **顶栏失衡与冗余红点 (图 2)**：顶栏标题（15px）字号偏小，弱于正文大标题；右侧工具按钮视觉分量过重；未保存状态常驻红色实心圆点，造成视觉压力。
4. **设置视窗排版断代 (图 1)**：当前设置页虽已去除 `border_1`，但整体版式仍为传统松散堆叠。参考图 1（Apple / Raycast 现代规范），亟需升级为带搜索的 Grouped Inset Card 分组嵌入卡片架构与 iOS 风格胶囊开关。
5. **代码编辑长行无法自适应折行且高亮单一 (图 2)**：源码编辑器仍为 `nowrap` 横向滚动，无法视窗软换行；语法高亮目前仅有单一 Accent 色，缺乏关键字、字符串等专业多色语义层次。

## 决策 (Decisions)

### 1. 纯净中性调色体系与去除未保存红点 (Ticket E1)
- **纯净中性底色重构（参考图 3）**：
  - 正文文本全面迁移至高级深碳灰（`#1f2328`），消除纯黑生硬感；
  - 行内代码与代码块背景改用清透极淡冷灰（`#f6f8fa`），行内代码文本使用中性深灰并包裹圆角微胶囊；
  - 代码块语言标签加粗、边框柔化，文本字重与对比度提升，消灭“太细”与“发脏”感。
- **清除未保存红点**：
  - 彻底移除顶栏大标题旁的未保存红色实心圆点；
  - 未保存保护完全交由退出应用或关闭文档时的模态防丢对话框承载，还原顶栏纯净度。

### 2. 侧栏两级层级、图标状态与 Hover 新建重构 (Ticket E2)
- **统一两级字体规范**：
  - 侧栏文字严格划分为两级：Level 1（Roots 与 Folders）为 13px Medium；Level 2（文件项）为 13px Regular。
- **去除三角箭头，改用文件夹状态图标**：
  - 彻底移除 `▸` 与 `▾` 字符箭头；
  - 文件夹展开/折叠状态直接通过开合的文件夹图标表达（`icons/folder.svg` 与 `icons/folder-open.svg`）。
- **Folder Hover 动作反转**：
  - 文件夹悬停右侧默认按钮重构为“+”（新建文件）；
  - 移除工作区根目录改由鼠标右键菜单或安全确认触发，彻底根绝误触。

### 3. 顶部标题栏权重升级与右侧工具轻量化 (Ticket E2)
- **大标题分量提升**：顶栏文档大标题字号提升至 18px Bold，确立清晰的主视觉锚点；
- **右侧工具栏轻量化**：工具图标尺寸微缩至 16px，采用低饱和微胶囊容器，削弱右侧权重，达成左重右轻的呼吸平衡。

### 4. 设置窗口升级为 Grouped Inset Card 架构 (Ticket E3)
- **左侧导航栏（参考图 1）**：
  - 顶部注入原生搜索框（`🔍 搜索`），支持实时过滤设置项；
  - 导航条目统一为现代轻灰圆角大胶囊（`rounded_lg`）。
- **右侧内容区分组嵌入卡片（Grouped Inset Cards）**：
  - 配置项按功能聚合在 `rounded_2xl` 圆角聚合卡片中；
  - 卡片内部各条目间以 1px 细发际线精准分隔；
  - 开关控件全面迁移至 iOS 风格平滑圆角胶囊开关（Pill Track + Toggle Thumb）。

### 5. 均高视觉行切分软折行与多色语义语法着色 (Ticket E4)
- **均高视觉行切分软折行 (Visual Line Wrapping)**：
  - 破局传统列表变高崩溃难题：在缓冲区渲染层将物理长行按视窗可用宽度切分为若干“视觉子行”；
  - 每一子行在 `uniform_list` 中占据完全相同的固定物理像素高度（`h(px(font_size * line_height))`）；
  - 仅首个子行显示物理行号，延续行行号列留空；
  - 光标移动与鼠标点击通过 `(physical_line, char_offset)` 精确双向映射。
- **多色语义语法分词着色 (Semantic Syntax Palette)**：
  - 升级 `tokenize_markdown_line` 支持丰富语义色彩：
    - 关键字（`fn`, `let`, `pub`, `struct`, `impl` 等）：优雅紫（`#8250df`）；
    - 字符串（`"..."`）：森林绿（`#1a7f37`）；
    - 各级标题（`#`）：深蓝/靛青（`#0969da`）；
    - 行内代码：清透青灰底配深深青字；
    - 注释（`//`、`<!-- -->`）：柔和弱灰（`#6e7781`）；
    - 链接：经典科技蓝。

## 任务拆解与推进节奏

- **Ticket E1 (纯净中性调色体系、代码样式洗净与去除未保存红点)**：
  - 范围：`crates/glidemark_app/src/theme.rs`, `crates/glidemark_app/src/views/preview.rs`, `crates/glidemark_app/src/views/workspace.rs`
  - 独立单测套件：`tests/clean_palette_and_code_style_test.rs`
- **Ticket E2 (侧栏两级层级、文件夹图标开合、新建文件 Hover 与 18px 顶栏大标题)**：
  - 范围：`crates/glidemark_app/src/views/drawer.rs`, `crates/glidemark_app/src/views/workspace.rs`
  - 独立单测套件：`tests/sidebar_two_tier_and_header_weight_test.rs`
- **Ticket E3 (设置视窗 Grouped Inset Cards 分组卡片与 iOS 胶囊开关重构)**：
  - 范围：`crates/glidemark_app/src/views/settings.rs`
  - 独立单测套件：`tests/settings_grouped_inset_test.rs`
- **Ticket E4 (均高视觉行切分软折行与多色语义语法高亮)**：
  - 范围：`crates/glidemark_app/src/views/editor.rs`, `crates/glidemark_app/src/editor_engine.rs`
  - 独立单测套件：`tests/visual_line_wrap_and_syntax_test.rs`

## 后果与影响 (Consequences)
- **正面收益**：
  - 界面整体视觉质感迈入现代顶级 macOS 应用梯队，彻底告别“脏色”与“粗笨”感；
  - 侧栏操作直觉安全，误删工作区风险彻底消除；
  - 设置页面 100% 贴合 Apple 官方现代设计语言；
  - 编辑器正式攻克软换行世纪难题，同时带来令人赏心悦目的多色代码着色。
- **架构约束与兼容**：
  - 软换行切分必须建立在非阻塞高效测量基础上，严格维持单帧微秒级预算；
  - 设置搜索过滤与分组卡片在全尺寸下具备完美弹性伸缩性。
