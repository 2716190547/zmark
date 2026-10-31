# GlideMark 领域词汇模型 (Domain Glossary)

GlideMark 是一款基于 **Rust + GPUI** 的高性能、侧栏驱动的多文件夹 Markdown 阅读与编辑器。
本文档定义统一的业务与架构术语，消灭交流歧义与冗余实现噪音。

---

## 核心领域术语 (Core Domain Terms)

### 1. Document (文档)
- **定义**：加载到当前活跃阅读或编辑会话中的单一本地 Markdown 物理文件。
- **避免叫法**：笔记 (Note)、卡片 (Card)、文章 (Article)、Vault Item

### 2. View Mode (视图模式)
- **定义**：文档内容的当前呈现形态，支持源码编辑（`Source`）、双栏分屏（`Split`）与纯排版阅读（`Preview`）三档切换。
- **避免叫法**：布局模式 (Layout Mode)、预览窗口 (Preview Window)

### 3. Buffer (文本缓冲区)
- **定义**：内存中存储 Markdown 原始 UTF-8 源码的文本数据结构，维护物理行索引与光标偏移量。
- **避免叫法**：文本框 (Textbox)、字符串缓存 (String Cache)

### 4. Render Tree (GPU 渲染树)
- **定义**：基于 Markdown 语法解析 AST 映射并实时提交给 GPUI/Metal 渲染管线的 GPU 原生元素树。
- **避免叫法**：DOM 树、WebView 页面、HTML 渲染图层

### 5. Block Element (块级图元)
- **定义**：Markdown 语法的离散顶级图元（段落、各级标题、独立表格、代码块、列表或流程图卡片）。
- **避免叫法**：小组件 (Widget)、卡片块 (Chunk)

### 6. Source Anchor (源码锚点)
- **定义**：附加于各渲染块级图元上的源码行范围元数据（Line Range），用于正文与源码之间的精准映射。
- **避免叫法**：行标记 (Line Tag)、书签 (Bookmark)

### 7. Sync Scroll (双向同步滚动)
- **定义**：分屏模式下基于源码锚点与分段线性插值、带有 100ms 回声抑制与输入防抖的双向平滑联动滚动机制。
- **避免叫法**：自动滚动 (Auto Scroll)、滚动绑定 (Scroll Bind)

### 8. Drawer (侧边抽屉)
- **定义**：承载多个工作区根、外部文件、草稿与文章大纲的侧边导航面板，文档的切换与打开均在此发生。
- **避免叫法**：侧边栏 (Sidebar)、文件资源管理器 (File Explorer)、目录树弹窗

### 9. Outline Navigation (大纲目录导航)
- **定义**：实时提取当前文档 H1–H6 标题构建的分级目录树，支持锚点平滑跳转与当前章节追踪。
- **避免叫法**：TOC 菜单、书签列表 (Bookmark List)

### 10. Active Document (当前文档)
- **定义**：右侧画布当前显示的文档，由侧栏文件树切换；同一时刻只有一个 Active Document，不再使用标签页表达。
- **避免叫法**：当前标签 (Current Tab)、前台文件 (Foreground File)

### 11. Workspace Root (工作区根)
- **定义**：用户显式添加到侧栏的本地根目录，可同时存在多个；每个根独立扫描、折叠与刷新。
- **避免叫法**：项目库 (Project Vault)、根仓库 (Root Repo)、工作区文件夹 (Workspace Folder)

### 12. Vector Diagram (矢量流程图)
- **定义**：由 100% 纯 Rust 渲染管线（`rusty-mermaid`）在内存中解析并绘制的 GPU 原生 Mermaid 矢量图元。
- **避免叫法**：WebKit 渲染图、外部 JS 视图、颜文字流程图

### 13. Preferences (偏好配置)
- **定义**：用户自定义的外观、调色板、编辑排版、自动保存与语言设置，持久化保存在标准 TOML 文件中。
- **避免叫法**：设置数据库 (Settings DB)、用户注册表

### 14. Palette (调色板)
- **定义**：经过专业调和的语义化色彩方案（包含默认暖石、Claude Warm、macOS Vibrant、Tokyo Night、Nord、GitHub、Solarized、One Dark）。
- **避免叫法**：皮肤 (Skin)、主题包 (Color Pack)

### 15. Editor Engine (自研编辑引擎)
- **定义**：基于纯文本行缓冲区的轻量编辑核心，精确管理光标位置、文本选区、按键流与撤销重做栈。
- **避免叫法**：IDE 插件、重型富文本框 (Rich Textarea)

### 16. Standalone Export (独立单文件导出)
- **定义**：将正文排版样式、矢量图表与代码块样式完全打包内联为自包含的单文件 `.html` 导出管线。
- **避免叫法**：PDF 导出、网页截屏 (Web Exporter)

### 17. Inline Rename (原地内联重命名)
- **定义**：双击右侧当前文档标题直接进入编辑态，并在确认后同步执行磁盘文件更名。
- **避免叫法**：另存为弹窗 (Save As Dialog)、重命名对话框

### 18. External Drop (系统原生文件拖拽)
- **定义**：捕获操作系统派发的文件拖放事件（`ExternalPaths`）并瞬时载入目标 Markdown 文档的原生通道。
- **避免叫法**：拖拽导入 (File Import)、拖动侦听器

### 19. App Bundle (macOS 原生独立应用包)
- **定义**：封装有 Universal/arm64 原生机器码、文件关联元数据（`Info.plist`）与官方多层资产（`Assets.car`）的标准桌面应用包。
- **避免叫法**：Electron 套壳包、安装包镜像 (DMG)

### 20. Auto Save (防抖自动落盘)
- **定义**：打字停止 1000ms 或视窗失焦时，后台自动将脏缓冲区静默持久化至物理磁盘的安全机制。
- **避免叫法**：定时轮询保存、云端同步

### 21. Loose File (外部文件)
- **定义**：已打开但不属于任何有效（非 missing）Workspace Root 的物理 Markdown 文件，归入侧栏“外部文件”虚拟组。
- **避免叫法**：游离文件、临时文件、未归档文件

### 22. Scratch Document (草稿文档)
- **定义**：没有物理路径的临时编辑文档，归入侧栏“草稿”虚拟组；首次保存时通过另存为确定路径。
- **避免叫法**：未命名标签、临时窗口、空文档

### 23. HiDPI Diagram Resampling (高分辨率矢量重采样)
- **定义**：针对流程图灯箱放大场景，采用 3600px 级大视口纹理预算进行矢量重采样的超清图形渲染管线，确保在 400% 极限缩放倍率下文字与几何线条零像素失真。
- **避免叫法**：位图拉伸 (Bitmap Scaling)、插值模糊 (Bilinear Blur)

### 24. Diagram Paper Canvas (灯箱图表纸张画布)
- **定义**：灯箱内承载 Mermaid 矢量图元的实体视窗图层，具备主题感知的不透明底色与高对比度边框，隔离底层半透明混合，杜绝底层正文穿透杂质。
- **避免叫法**：透明图层、透光蒙版

### 25. Soft Wrap (编辑行软折行)
- **定义**：源码编辑器在保持物理行号严格递增的前提下，长文本到达视窗边界时自适应视觉折行，兼顾阅读舒适度与光标点击精确定位。
- **避免叫法**：硬回车折行、强制换行 (Hard Break)

### 26. Markdown Syntax Tokenizer (轻量 Markdown 语法着色器)
- **定义**：在编辑行渲染时由轻量词法分析器分词并赋予语义化调色板色彩（如标题、引用、行内代码、链接与加粗）的高性能着色管线。
- **避免叫法**：IDE 插件着色、WebView 高亮


