# GlideMark 领域词汇模型 (Domain Glossary)

GlideMark 是一款基于 **Rust + GPUI** 的高性能单窗口多标签 GPU 加速 Markdown 阅读与编辑器。
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
- **定义**：承载同级工作区文件与文章大纲的可折叠侧边导航面板，支持 180ms 硬件加速平滑动效。
- **避免叫法**：侧边栏 (Sidebar)、文件资源管理器 (File Explorer)、目录树弹窗

### 9. Outline Navigation (大纲目录导航)
- **定义**：实时提取当前文档 H1–H6 标题构建的分级目录树，支持锚点平滑跳转与当前章节追踪。
- **避免叫法**：TOC 菜单、书签列表 (Bookmark List)

### 10. Document Tab (多标签页)
- **定义**：单窗口内代表独立打开文档会话的轻量交互标签，支持一键切换、新建与快捷键关闭。
- **避免叫法**：文件胶囊 (File Pill)、窗口按钮 (Window Tag)

### 11. Workspace Folder (工作区文件夹)
- **定义**：用户选定载入的本地根目录，系统自动扫描并索引其内部所有 Markdown 关联文档。
- **避免叫法**：项目库 (Project Vault)、根仓库 (Root Repo)

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
- **定义**：双击顶栏文件名直接进入编辑态并在确认后同步执行磁盘文件更名的无感操作。
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
