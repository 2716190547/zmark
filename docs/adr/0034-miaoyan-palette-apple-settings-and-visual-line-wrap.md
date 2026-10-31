# ADR 0034: 纯净中性调色体系、Apple 原生分组设置与均高切分软折行

## 状态
已通过 (Accepted) - 由审查端复审通过，进入分步实施阶段 (Ticket E1 ~ E4)

## 上下文 (Context)
在 Phase D（设置微胶囊化与基础阅读排版）落地后，针对用户提交的 3 张视觉标注与参考设计（图 1 设置排版、图 2 现状痛点、图 3 妙言清爽排版）进行了系统性工程审视：
1. **颜色发脏与行内代码黄褐混杂 (图 2 & 图 3)**：当前默认主题存在黄褐色杂质（如 `code_bg` 为 `#EEECE6`），行内代码叠加 `accent`（橙红）后整体观感发脏；代码块标签与文本偏细发灰。参考图 3（妙言），亟需全面重构为纯净中性色。
2. **侧栏层级杂乱与误触风险 (图 2)**：侧栏字体大小未统一，三角标（`▸`/`▾`）与文件夹图标重复冗余；文件夹 Hover 默认展示删除按钮（`×`），极易误删工作区根目录。
3. **顶栏失衡与冗余红点 (图 2)**：顶栏标题（15px）字号偏小，弱于正文大标题；右侧工具按钮视觉分量过重；未保存状态常驻红色实心圆点，造成视觉压力。
4. **设置视窗排版断代 (图 1)**：当前设置页虽已去除 `border_1`，但整体版式仍为传统松散堆叠。参考图 1（Apple / Raycast 现代规范），亟需升级为带搜索的 Grouped Inset Card 分组嵌入卡片架构与 iOS 风格胶囊开关。
5. **代码编辑长行无法自适应折行且高亮单一 (图 2)**：源码编辑器仍为 `nowrap` 横向滚动，无法视窗软换行；语法高亮目前仅有单一 Accent 色，缺乏关键字、字符串等专业多色语义层次与 fenced code 上下文解析。

## 决策 (Decisions)

### 1. 纯净中性调色体系、8 套主题全量语义 Token 矩阵与去除未保存红点 (Ticket E1)
- **纯净中性底色重构（参考图 3 妙言）**：
  - 正文文本全面迁移至高级深碳灰（`#1f2328`），消除纯黑生硬感；
  - 行内代码与代码块背景改用清透极淡冷灰（`#f6f8fa`），行内代码文本使用中性深灰并包裹圆角微胶囊；
  - 代码块语言标签加粗、边框柔化，文本字重与对比度提升，消灭“太细”与“发脏”感。
- **8 套调色板全量多色语义高亮色彩矩阵 (Full Syntax Token Matrix)**：
  - 在 `ThemeColors` 中扩展 6 项语法高亮颜色字段：
    - `syntax_keyword`: 关键字（`fn`, `let`, `pub`, `struct`, `impl` 等）
    - `syntax_string`: 字符串（`"..."`）
    - `syntax_comment`: 单行与块级注释（`//`, `<!-- -->`）
    - `syntax_heading`: 标题（各级 `#`）
    - `syntax_inline_code`: 行内代码文本色
    - `syntax_link`: 链接标记与地址
  - 8 套调色板在浅色与暗色模式下的完整色彩定义矩阵如下：

| 调色板 ID | 模式 | 关键字 (keyword) | 字符串 (string) | 注释 (comment) | 标题 (heading) | 行内代码 (code) | 链接 (link) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **default** (妙言中性) | 浅色 | `#8250df` (优雅紫) | `#1a7f37` (森林绿) | `#6e7781` (柔弱灰) | `#0969da` (科技蓝) | `#1f2328` (深碳灰) | `#0969da` (经典蓝) |
| | 暗色 | `#d2a8ff` (浅紫) | `#7ee787` (青草绿) | `#8b949e` (中灰) | `#58a6ff` (天蓝) | `#f3f1ed` (暖白) | `#58a6ff` (天蓝) |
| **claude** (Claude 暖色) | 浅色 | `#d97706` (琥珀黄) | `#059669` (翡翠绿) | `#78716c` (暖石灰) | `#b45309` (赤褐棕) | `#292524` (黑棕) | `#b45309` (暖棕) |
| | 暗色 | `#fbbf24` (明黄暖金) | `#34d399` (薄荷绿) | `#a8a29e` (浅灰石) | `#f59e0b` (暖金) | `#fafaf9` (奶白) | `#fbbf24` (明黄) |
| **macos** (macOS 活力) | 浅色 | `#af52de` (系统紫) | `#34c759` (系统绿) | `#8e8e93` (系统灰) | `#007aff` (系统蓝) | `#1c1c1e` (系统黑) | `#007aff` (系统蓝) |
| | 暗色 | `#bf5af2` (暗紫) | `#30d158` (暗绿) | `#98989d` (暗灰) | `#0a84ff` (暗蓝) | `#ffffff` (纯白) | `#0a84ff` (暗蓝) |
| **tokyo-night** (东京夜) | 浅色 | `#7847bd` (紫罗兰) | `#485e30` (橄榄绿) | `#8990b3` (暮色灰) | `#2e7de9` (靛蓝) | `#343b58` (深蓝灰) | `#2e7de9` (靛蓝) |
| | 暗色 | `#bb9af7` (霓虹紫) | `#9ece6a` (荧光绿) | `#565f89` (夜雾灰) | `#7aa2f7` (冰蓝) | `#c0caf5` (冰白) | `#7dcfff` (青空蓝) |
| **nord** (北欧极光) | 浅色 | `#b48ead` (极光紫) | `#a3be8c` (苔原绿) | `#4c566a` (暗板岩) | `#5e81ac` (极地蓝) | `#2e3440` (夜黑) | `#88c0d0` (霜蓝) |
| | 暗色 | `#b48ead` (极光紫) | `#a3be8c` (苔原绿) | `#616e88` (板岩灰) | `#81a1c1` (冰川蓝) | `#eceff4` (雪白) | `#88c0d0` (霜冻蓝) |
| **github** (GitHub 官方) | 浅色 | `#cf222e` (GitHub红) | `#0a3069` (深邃蓝) | `#6e7781` (中灰) | `#0550ae` (标称蓝) | `#24292f` (墨黑) | `#0969da` (链接蓝) |
| | 暗色 | `#ff7b72` (珊瑚红) | `#a5d6ff` (浅蓝) | `#8b949e` (暗灰) | `#79c0ff` (亮蓝) | `#c9d1d9` (浅灰) | `#58a6ff` (高亮蓝) |
| **solarized** (日光温和) | 浅色 | `#859900` (日光绿) | `#2aa198` (青色) | `#93a1a1` (基底灰) | `#268bd2` (深蓝) | `#073642` (暗青) | `#6c71c4` (堇紫) |
| | 暗色 | `#859900` (日光绿) | `#2aa198` (青色) | `#586e75` (暗灰) | `#268bd2` (深蓝) | `#93a1a1` (亮青灰) | `#6c71c4` (堇紫) |
| **one-dark** (One Dark) | 浅色 | `#a626a4` (深品红) | `#50a14f` (深草绿) | `#a0a1a7` (烟灰) | `#4078f2` (One 蓝) | `#383a42` (纯净黑) | `#4078f2` (One 蓝) |
| | 暗色 | `#c678dd` (亮紫红) | `#98c379` (草绿) | `#5c6370` (暗灰) | `#61afef` (明亮蓝) | `#abb2bf` (银白) | `#61afef` (明亮蓝) |

- **清除未保存红点**：
  - 彻底移除顶栏大标题旁的未保存红色实心圆点；
  - 未保存保护完全交由退出应用或关闭文档时的模态防丢对话框承载，还原顶栏纯净度。

### 2. 侧栏两级层级、开合图标、新建文件 Hover 与 AppAssets 静态资产注册 (Ticket E2)
- **统一两级字体规范**：
  - 侧栏文字严格划分为两级：Level 1（Roots 与 Folders）统一为 13px Medium；Level 2（文件项）统一为 13px Regular。
- **去除三角箭头与引入开合状态图标**：
  - 彻底移除 `▸` 与 `▾` 字符箭头；
  - 引入新资产 `crates/glidemark_app/assets/icons/folder-open.svg`；
  - **AppAssets 静态加载注册**：在 `crates/glidemark_app/src/assets.rs` 的 `AppAssets::load()` 中注册 `"icons/folder-open.svg" => Some(include_bytes!(...))`，并在 `AppAssets::list()` 中追加条目，确保通过静态资产加载与 `test_assets_load_and_list` 单测；
  - 文件夹展开态渲染 `folder-open.svg`，折叠态渲染 `folder.svg`。
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

### 4. 设置窗口升级为 Grouped Inset Card 架构、原生搜索与 10 语言国际化 (Ticket E3)
- **搜索栏资产与 AppAssets 注册**：
  - 引入原生矢量资产 `crates/glidemark_app/assets/icons/search.svg`，彻底摒弃 Unicode Emoji 搜索符号；
  - **AppAssets 静态加载注册**：在 `crates/glidemark_app/src/assets.rs` 的 `AppAssets::load()` 中注册 `"icons/search.svg" => Some(include_bytes!(...))`，并在 `AppAssets::list()` 中追加条目；
- **10 语言全量国际化**：
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
- **左侧导航栏与右侧 Grouped Inset Cards**：
  - 左侧顶部注入带 `search.svg` 的原生搜索框，支持实时过滤设置条目与高亮；
  - 导航条目统一为现代轻灰圆角大胶囊（`rounded_lg`）；
  - 右侧配置项按功能聚合在 `rounded_2xl` 圆角聚合卡片中，条目间以 1px 细发际线分隔；开关控件全面迁移至 iOS 风格平滑圆角胶囊开关（Pill Track + Toggle Thumb）。

### 5. 均高视觉行切分软折行、Fenced Code 多色语法高亮与 Workspace 双向协同 (Ticket E4)

#### A. 均高视觉行切分软折行架构 (Visual Line Wrapping)
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
- **等宽 CJK/Emoji 宽度测量 API**：
  - 西文（ASCII）字符显示单元宽为 1；
  - 中日韩全角字符（CJK）、Emoji、全角标点使用宽度测量算法计为 2 个单元宽；
  - 基准西文字符像素宽度：`unit_width = font_size * 0.6`；
  - 视窗单行最大单元列容量：`max_cols = ((viewport_width - gutter_width) / unit_width).floor() as usize`（其中 `gutter_width` 包含行号列宽 36px、边距及右侧内边距约 68px）；
  - 逐字累加单元宽，当超过 `max_cols` 时折断生成下一个 `VisualLine`（优先在空格/标点词边界切分，不可分长词执行硬切分）。
- **横向滚动彻底禁用与视窗边界约束**：
  - 移除原 `ListHorizontalSizingBehavior::Unconstrained`，改用视窗约束；
  - 移除每一行的 `min_w_full()` 与 `whitespace_nowrap()` 导致的无限制水平溢出，每行视觉子行宽度严格贴合可用视窗宽度；
  - 移除水平滚动偏移计算，`scroll_x` 置为 0.0，彻底消除横向滚动条与横向晃动。
- **缓存失效边界 (Cache Invalidation Boundaries)**：
  - `VisualLineCache` 仅在以下 3 类明确边界事件发生时失效并重新测量切分：
    1. **内容变更**：`buffer_revision` 变动（用户输入、删除、粘贴、撤销、重做、磁盘重载）；
    2. **视窗排版变更**：`viewport_width` 变动（主窗口 resize、侧栏折叠/展开、Split Divider 拖动）；
    3. **偏好参数变更**：`font_size` 或 `line_height` 改变。
  - 平时滚动与光标闪烁绝不触发重新测量切分，渲染帧内以 0 分配只读检索，严格维持单帧微秒级预算。
- **双向坐标映射与 Workspace 协同 (包含 workspace.rs)**：
  - **光标正向映射**：输入物理坐标 `(phys_line, phys_col)`，定位所属 `visual_line_idx` 及视觉子行内偏移 `sub_char_offset`；
  - **点击逆向命中**：鼠标在编辑区点击 `(x, y)`，由 `vis_idx = (y / row_height) + scroll_top` 定位视觉行，`x - gutter_width` 反查字符，逆向换算为物理光标 `(phys_line, char_start + sub_char_offset)` 并派发给编辑引擎；
  - **垂直导航 (Up/Down)**：光标在 `vis_idx ± 1` 视觉空间垂直移动，保持目标视觉横坐标 `target_vis_x`，逆向映射回对应物理行列；
  - **Split View 同步滚动换算 (`workspace.rs`)**：
    - `workspace.rs` 的 `handle_editor_scroll_wheel` 严禁直接将 `scroll_y / line_height` 作为物理行号；
    - 必须通过 `visual_lines[vis_idx].physical_line_idx` 精确查表获取真实物理行号与子行进度，再传给 `SyncScrollMap::source_line_to_preview_progress`，彻底杜绝 Split 模式下预览错位。

#### B. Fenced Code 代码块上下文追踪与多色语法分词
- **语法状态机**：
  - 解析遍历时追踪 `in_fenced_code: Option<&str>` 状态，以行首 ``` 及语言标示（如 ```rust）切换上下文；
  - **代码块内部**：按编程语言词法规则分词，将关键字（`fn`, `let`, `pub`, `struct`, `impl` 等）、字符串（`"..."`）、单行注释（`//`）着色为对应的 `syntax_keyword`、`syntax_string`、`syntax_comment`；
  - **普通 Markdown 行**：按 Markdown 语法分词，将标题着色为 `syntax_heading`、行内代码包裹微胶囊并着色 `syntax_inline_code`、链接着色 `syntax_link`。

## 任务拆解与推进节奏

- **Ticket E1 (纯净中性调色体系、8 套主题语义 Token 矩阵与去除未保存红点)**：
  - 涉及文件：
    - `crates/glidemark_app/src/theme.rs`
    - `crates/glidemark_app/src/views/preview.rs`
    - `crates/glidemark_app/src/views/workspace.rs`
  - 独立测试套件：`crates/glidemark_app/tests/clean_palette_and_code_style_test.rs`
- **Ticket E2 (侧栏两级层级、开合图标、新建文件 Hover、顶栏 18px 标题、76px 安全区避让与 AppAssets 注册)**：
  - 涉及文件：
    - `crates/glidemark_app/assets/icons/folder-open.svg` (新增)
    - `crates/glidemark_app/src/assets.rs` (注册 folder-open.svg)
    - `crates/glidemark_app/src/state.rs`
    - `crates/glidemark_app/src/views/drawer.rs`
    - `crates/glidemark_app/src/views/workspace.rs`
  - 独立测试套件：`crates/glidemark_app/tests/sidebar_two_tier_and_header_weight_test.rs`
- **Ticket E3 (设置视窗 Grouped Inset Cards 分组卡片、iOS 胶囊开关、10 语言搜索与 AppAssets 注册)**：
  - 涉及文件：
    - `crates/glidemark_app/assets/icons/search.svg` (新增)
    - `crates/glidemark_app/src/assets.rs` (注册 search.svg)
    - `crates/glidemark_core/src/i18n.rs`
    - `crates/glidemark_app/src/views/settings.rs`
  - 独立测试套件：`crates/glidemark_app/tests/settings_grouped_inset_test.rs`
- **Ticket E4 (均高视觉行切分软折行、Fenced Code 多色语法高亮与 Workspace 映射协同)**：
  - 涉及文件：
    - `crates/glidemark_app/src/views/editor.rs`
    - `crates/glidemark_app/src/editor_engine.rs`
    - `crates/glidemark_app/src/views/workspace.rs`
  - 独立测试套件：`crates/glidemark_app/tests/visual_line_wrap_and_syntax_test.rs`

## 后果与影响 (Consequences)
- **正面收益**：
  - 界面整体视觉质感迈入现代顶级 macOS 应用梯队，彻底告别“脏色”与“粗笨”感；
  - 侧栏操作直觉安全，误删工作区根目录的风险彻底消除；
  - 设置页面 100% 贴合 Apple 官方现代设计语言，原生搜索支持 10 种国际化语言；
  - 编辑器完整攻克 CJK/Emoji 软换行与坐标映射难题，横向滚动彻底禁用，同时支持 Fenced Code 与 Markdown 双重语境多色语法着色；
  - `workspace.rs` 实现精确物理行反查，Split View 同步滚动丝滑对齐；
  - 严格保持 macOS 76px 顶栏红绿灯物理避让，无任何遮挡风险。
- **架构约束与兼容**：
  - 软换行切分与测量必须依赖 `VisualLineCache`，杜绝渲染循环中的重复测量；
  - `uniform_list` 物理行高恒定规则坚决保持，所有视觉子行均维持 `h(px(font_size * line_height))`。
