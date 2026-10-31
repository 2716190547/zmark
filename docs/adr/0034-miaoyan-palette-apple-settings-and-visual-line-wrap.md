# ADR 0034: 纯净中性调色体系、Apple 原生分组设置与均高切分软折行

## 状态
提议 (Proposed) - 等待 Ticket E1、E2、E3、E4 分步实施验收后转为 Accepted

## 上下文 (Context)
在 Phase D（设置微胶囊化与基础阅读排版）落地后，针对用户提交的 3 张视觉标注与参考设计（图 1 设置排版、图 2 现状痛点、图 3 妙言清爽排版）进行了系统性工程审视：
1. **颜色发脏与行内代码黄褐混杂 (图 2 & 图 3)**：当前默认主题存在黄褐色杂质（如 `code_bg` 为 `#EEECE6`），行内代码叠加 `accent`（橙红）后整体观感发脏；代码块标签与文本偏细发灰。参考图 3（妙言），亟需全面重构为纯净中性色。
2. **侧栏层级杂乱与误触风险 (图 2)**：侧栏字体大小未统一，三角标（`▸`/`▾`）与文件夹图标重复冗余；文件夹 Hover 默认展示删除按钮（`×`），极易误删工作区根目录。
3. **顶栏失衡与冗余红点 (图 2)**：顶栏标题（15px）字号偏小，弱于正文大标题；右侧工具按钮视觉分量过重；未保存状态常驻红色实心圆点，造成视觉压力。
4. **设置视窗排版断代 (图 1)**：当前设置页虽已去除 `border_1`，但整体版式仍为传统松散堆叠。参考图 1（Apple / Raycast 现代规范），亟需升级为带搜索的 Grouped Inset Card 分组嵌入卡片架构与 iOS 风格胶囊开关。
5. **代码编辑长行无法自适应折行且高亮单一 (图 2)**：源码编辑器仍为 `nowrap` 横向滚动，无法视窗软换行；语法高亮目前仅有单一 Accent 色，缺乏关键字、字符串等专业多色语义层次与 fenced code 上下文解析。

## 决策 (Decisions)

### 1. 纯净中性调色体系、多色语义 Token 与去除未保存红点 (Ticket E1)
- **纯净中性底色重构（参考图 3 妙言）**：
  - 正文文本全面迁移至高级深碳灰（`#1f2328`），消除纯黑生硬感；
  - 行内代码与代码块背景改用清透极淡冷灰（`#f6f8fa`），行内代码文本使用中性深灰并包裹圆角微胶囊；
  - 代码块语言标签加粗、边框柔化，文本字重与对比度提升，消灭“太细”与“发脏”感。
- **8 套调色板全量多色语义高亮 Token (Syntax Tokens)**：
  - 在 `ThemeColors` 中扩展 6 项语义高亮颜色字段：
    - `syntax_keyword`: 关键字色（如 `fn`, `let`, `pub`, `struct`, `impl`）
    - `syntax_string`: 字符串色（如 `"..."`）
    - `syntax_comment`: 注释色（如 `//`, `<!-- -->`）
    - `syntax_heading`: 标题色（各级 `#`）
    - `syntax_inline_code`: 行内代码文本色
    - `syntax_link`: 链接色
  - 8 套调色板（`default`, `claude`, `macos`, `tokyo-night`, `nord`, `github`, `solarized`, `one-dark`）在浅色与暗色模式下全部注入专属和谐色彩矩阵（例如 Default 浅色关键字采用优雅紫 `#8250df`，字符串采用森林绿 `#1a7f37`，注释采用柔和冷灰 `#6e7781`；Default 暗色模式采用对应高可读性柔光色）。
- **清除未保存红点**：
  - 彻底移除顶栏大标题旁的未保存红色实心圆点；
  - 未保存保护完全交由退出应用或关闭文档时的模态防丢对话框承载，还原顶栏纯净度。

### 2. 侧栏两级层级、图标状态与 Hover 新建重构 (Ticket E2)
- **统一两级字体规范**：
  - 侧栏文字严格划分为两级：Level 1（Roots 与 Folders）统一为 13px Medium；Level 2（文件项）统一为 13px Regular。
- **去除三角箭头，改用文件夹状态图标**：
  - 彻底移除 `▸` 与 `▾` 字符箭头；
  - 引入新资产 `crates/glidemark_app/assets/icons/folder-open.svg`；
  - 文件夹展开态展示 `folder-open.svg`，折叠态展示 `folder.svg`。
- **Folder Hover 动作反转与右键防误删**：
  - 文件夹/根目录悬停（Hover）右侧默认按钮重构为“+”（新建文件）；点击时调用 `on_new_file_in_dir` 创建并激活未命名文件；
  - 移除根目录操作改为在根目录项上触发鼠标右键（`MouseButton::Right`）二次确认菜单，彻底根绝鼠标悬停误点击 `×` 导致工作区意外移除。

### 3. 顶部标题栏权重升级、右侧工具轻量化与 macOS 76px 安全区避让 (Ticket E2)
- **大标题分量提升**：顶栏文档大标题字号由 15px 提升至 18px Bold，确立清晰的主视觉锚点；
- **右侧工具栏轻量化**：工具图标尺寸微缩至 16px，采用低饱和微胶囊容器，削弱右侧权重，达成左重右轻的呼吸平衡；
- **macOS 76px 安全区强约束（继承 ADR 0029 / 0033）**：
  - macOS 顶栏左侧强制避让系统 76px 红绿灯控制区；
  - 侧栏折叠态（Drawer 关闭）：顶栏大标题起始偏移锁定为 118px（76px 红绿灯 + 展开按钮与边距），严禁反向左侵红绿灯区；
  - 侧栏展开态（Drawer 打开）：顶栏大标题水平偏移精确对齐编辑器 64px 第一列。

### 4. 设置窗口升级为 Grouped Inset Card 架构与原生搜索 (Ticket E3)
- **搜索栏资产与 10 语言全量国际化**：
  - 引入原生矢量资产 `crates/glidemark_app/assets/icons/search.svg`，彻底摒弃 Unicode Emoji `🔍`；
  - 在 `crates/glidemark_core/src/i18n.rs` 的 `I18nStrings` 中扩充 `search_settings` 字段，并在 10 种语言中完整定义本地化文案：
    - `ZhCn`: "搜索设置"
    - `ZhTw`: "搜尋設定"
    - `En`: "Search Settings"
    - `Ja`: "設定を検索"
    - `Ko`: "설정 검색"
    - `Fr`: "Rechercher dans les paramètres"
    - `De`: "Einstellungen durchsuchen"
    - `Es`: "Buscar en ajustes"
    - `Ru`: "Поиск настроек"
    - `Pt`: "Pesquisar nas definições"
- **左侧导航栏（参考图 1）**：
  - 顶部注入带 `search.svg` 的原生搜索框，支持实时过滤设置条目与高亮；
  - 导航条目统一为现代轻灰圆角大胶囊（`rounded_lg`）。
- **右侧内容区分组嵌入卡片（Grouped Inset Cards）**：
  - 配置项按功能聚合在 `rounded_2xl` 圆角聚合卡片中；
  - 卡片内部各条目间以 1px 细发际线精准分隔；
  - 开关控件全面迁移至 iOS 风格平滑圆角胶囊开关（Pill Track + Toggle Thumb）。

### 5. 均高视觉行切分软折行与 Fenced Code 多色语法高亮 (Ticket E4)

#### A. 均高视觉行切分软折行模型 (Visual Line Wrapping)
- **数据结构定义**：
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct VisualLine {
      pub physical_line_idx: usize,
      pub subline_idx: usize,    // 0 为物理行首，>0 为后续视觉折行
      pub char_start: usize,     // 在物理行中的字符起始 offset
      pub char_len: usize,       // 视觉行内包含的字符数
      pub is_first: bool,        // 是否显示物理行号
  }
  ```
- **CJK / Emoji 宽度测量与切分算法**：
  - 西文（ASCII）字符显示宽度计为 1 个单元宽；
  - 中日韩全角字符（CJK）、Emoji、全角符号通过字符宽度测量计为 2 个单元宽；
  - 根据编辑器视窗可用像素宽度 `viewport_width`（减去边栏宽度 68px 与内边距），换算出单行最大单元列容量 `max_units = (viewport_width - gutter_width) / unit_char_width`；
  - 逐字累加单元宽，当超过 `max_units` 时分割生成下一个 `VisualLine`（优先在空格/标点词边界切分，不可分长词执行硬切分）。
- **双向坐标映射机制**：
  - **光标正向映射**：输入物理坐标 `(phys_line, phys_col)`，定位所属 `visual_line_idx` 及视觉子行内偏移 `sub_char_offset`；
  - **点击逆向命中**：鼠标点击 `(x, y)` 经由 `vis_idx = (y / row_height) + scroll_top` 检索视觉行，结合 `x` 偏移反查字符，逆向换算为物理光标 `(phys_line, char_start + sub_char_offset)`；
  - **垂直导航 (Up/Down)**：光标在 `vis_idx ± 1` 视觉空间垂直移动，保持目标视觉横坐标 `target_vis_x`，逆向映射回对应物理行列；
  - **同步滚动映射**：预览区与编辑器基于当前可见顶端 `VisualLine.physical_line_idx` 加子行比例进行平滑换算，杜绝 Split View 同步滚动跳变。
- **宽度变化与内容变更缓存 (`VisualLineCache`)**：
  - 快取键：`(buffer_revision, max_units)`；
  - 缓存内容：`visual_lines: Vec<VisualLine>` 与物理行速查表 `phys_to_first_vis: Vec<usize>`；
  - 当视窗尺寸改变（resize）或文本内容变更时失效重建，渲染帧期间无锁只读极速查表，严格维持单帧微秒级预算。

#### B. Fenced Code 代码块上下文追踪与多色语法分词
- **语法状态机**：
  - 解析遍历时追踪 `in_fenced_code: Option<&str>` 状态，以行首 ``` 及语言标示（如 ```rust）切换上下文；
  - **代码块内部**：按编程语言词法规则分词，将关键字（`fn`, `let`, `pub`, `struct`, `impl` 等）、字符串（`"..."`）、单行注释（`//`）着色为对应的 `syntax_keyword`、`syntax_string`、`syntax_comment`；
  - **普通 Markdown 行**：按 Markdown 语法分词，将标题着色为 `syntax_heading`、行内代码包裹微胶囊并着色 `syntax_inline_code`、链接着色 `syntax_link`。

## 任务拆解与推进节奏

- **Ticket E1 (纯净中性调色体系、多色语法 Token 与去除未保存红点)**：
  - 涉及文件：
    - `crates/glidemark_app/src/theme.rs`
    - `crates/glidemark_app/src/views/preview.rs`
    - `crates/glidemark_app/src/views/workspace.rs`
  - 独立测试套件：`crates/glidemark_app/tests/clean_palette_and_code_style_test.rs`
- **Ticket E2 (侧栏两级层级、开合图标、新建文件 Hover、顶栏 18px 标题与 76px 安全区避让)**：
  - 涉及文件：
    - `crates/glidemark_app/assets/icons/folder-open.svg` (新增)
    - `crates/glidemark_app/src/state.rs`
    - `crates/glidemark_app/src/views/drawer.rs`
    - `crates/glidemark_app/src/views/workspace.rs`
  - 独立测试套件：`crates/glidemark_app/tests/sidebar_two_tier_and_header_weight_test.rs`
- **Ticket E3 (设置视窗 Grouped Inset Cards 分组卡片、iOS 胶囊开关与 10 语言搜索)**：
  - 涉及文件：
    - `crates/glidemark_app/assets/icons/search.svg` (新增)
    - `crates/glidemark_core/src/i18n.rs`
    - `crates/glidemark_app/src/views/settings.rs`
  - 独立测试套件：`crates/glidemark_app/tests/settings_grouped_inset_test.rs`
- **Ticket E4 (均高视觉行切分软折行与 Fenced Code 多色语法高亮)**：
  - 涉及文件：
    - `crates/glidemark_app/src/views/editor.rs`
    - `crates/glidemark_app/src/editor_engine.rs`
  - 独立测试套件：`crates/glidemark_app/tests/visual_line_wrap_and_syntax_test.rs`

## 后果与影响 (Consequences)
- **正面收益**：
  - 界面整体视觉质感迈入现代顶级 macOS 应用梯队，彻底告别“脏色”与“粗笨”感；
  - 侧栏操作直觉安全，误删工作区根目录的风险彻底消除；
  - 设置页面 100% 贴合 Apple 官方现代设计语言，原生搜索支持 10 种国际化语言；
  - 编辑器完整攻克 CJK/Emoji 软换行与坐标映射难题，同时支持 Fenced Code 与 Markdown 双重语境多色语法着色；
  - 严格保持 macOS 76px 顶栏红绿灯物理避让，无任何遮挡风险。
- **架构约束与兼容**：
  - 软换行切分与测量必须依赖 `VisualLineCache`，杜绝渲染循环中的重复测量；
  - `uniform_list` 物理行高恒定规则坚决保持，所有视觉子行均维持 `h(px(font_size * line_height))`。
