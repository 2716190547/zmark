# GlideMark 架构决策记录 (ADR 索引)

本文档是 GlideMark 架构演进与技术选型的唯一权威索引表。
所有技术决策遵循轻量、务实、直面根因原则，早期已被替代的中间态探索已并入下表所列去向，不保留冗余文件。

---

## 一、 有效架构决策清单

| 编号 | 标题 | 状态 | 核心决策与结论 |
| :--- | :--- | :--- | :--- |
| **0001** | [核心技术栈与基础架构](0001-core-tech-stack-and-architecture.md) | 已接受 (Accepted) | 采用 Rust + GPUI 渲染引擎与自研纯文本编辑核心 |
| **0002** | [虚拟化块渲染与同步滚动架构](0002-virtualized-block-rendering-and-sync-scrolling.md) | 已接受 (Accepted) | 确立源码锚点与分块渲染模型；双向同步在 0028 落地 |
| **0003** | [自包含 HTML 独立单文件导出](0003-self-contained-html-export.md) | 已接受 (Accepted) | 导出内联完整 CSS 样式的单文件离线 HTML，无需依赖外部浏览器 |
| **0004** | [Core 与 App 双 Crate 解耦边界](0004-core-ui-crate-decoupling.md) | 已接受 (Accepted) | 逻辑解析（glidemark_core）与 GPU 渲染层（glidemark_app）严格隔离 |
| **0005** | [自研编辑引擎与多调色板体系](0005-unified-topbar-custom-editor-and-palette-system.md) | 已接受 (Accepted) | 自研轻量纯文本行光标引擎，杜绝重型外部编辑器依赖 |
| **0006** | [异步非阻塞文件选择与拖拽集成](0006-async-file-dialog-and-external-drop-handling.md) | 已接受 (Accepted) | 原生异步对话框杜绝 AppKit 主线程借用死锁，支持拖拽即开 |
| **0007** | [Flexbox 滚动约束与布局根因修复](0007-flexbox-scroll-constraints.md) | 已接受 (Accepted) | 明确 `min-h: 0` 与 `min-w: 0` 约束，彻底根治 Taffy 弹性盒溢出滚动卡死 |
| **0009** | [Tooltip 规范、去边线与平滑动效](0009-tooltips-borderless-minimalism-rename-fix-and-animations.md) | 已接受 (Accepted) | 全局无框化微胶囊、双击原地重命名与 180ms 原生平滑动效 |
| **0013** | [解耦独立分栏设置窗口与平滑动效](0013-decoupled-split-settings-window-bidirectional-animations-and-fullbleed-icon.md) | 已接受 (Accepted) | 独立设置视窗、双向跨窗口即时同步、1s 防抖自动落盘（合并 0010/0011/0012） |
| **0015** | [Icon Composer 资产与 Squircle 规范](0015-icon-composer-iconimagestack-bypass-squircle-jail.md) | 已接受 (Accepted) | 编译标准 `Assets.car` 与 `IconImageStack`，彻底根治 macOS Dock 灰色托盘套娃 |
| **0019** | [采用 100% 纯 Rust 图表渲染引擎](0019-pure-rust-mermaid-rendering-engine-adoption.md) | 已接受 (Accepted) | 选用 `rusty-mermaid` 替换 WebKit / JS 管道，彻底肃清外部通信与 JS 依赖 |
| **0020** | [SVG 视口自适应几何归一化](0020-svg-bounding-box-normalization-and-void-elimination.md) | 已接受 (Accepted) | 计算图元最小包围盒并重写 viewBox，彻底消灭复杂子图顶部几何大空白 |
| **0021** | [单窗口多标签页与工作区文件夹架构](0021-multi-tab-and-workspace-directory-architecture.md) | 已接受 (Accepted) | 多标签资源模型保留；Tab Strip 由 ADR 0031 取代，暖石主题仍有效 |
| **0022** | [图表纹理预算与有界 LRU 缓存](0022-diagram-texture-budget-and-memory-optimization.md) | 已接受 (Accepted) | 建立 1000px 尺寸钳制与 LRU 显存逐出淘汰机制，彻底终结超大图表显存膨胀 |
| **0024** | [图表 Panic 隔离与编辑器横向平滑滚动](0024-panic-isolation-native-dialogs-and-editor-horizontal-scroll.md) | 已接受 (Accepted) | 纯 Rust 图表 Panic 隔离屏障，编辑器长行采用 `nowrap` 与横向双向平滑滚动 |
| **0026** | [大文档虚拟化与文件体积三级解析分档](0026-large-document-virtualization-parse-tiers-and-deferred-sync-scroll.md) | 已接受 (Accepted) | GPUI 原生 `list` 视口按需虚拟化，按 2MB/10MB 分级防抖，大文档内存降至百 MB 级 |
| **0027** | [双板块无界纸张画布与图表灯箱](0027-dual-pane-seamless-canvas-and-diagram-lightbox.md) | 已接受 (Accepted) | 去除全局通栏顶栏、双板块画布、工作区树与矢量灯箱；红绿灯为静态位置 + 76px 安全区（见 0029） |
| **0028** | [双向同步滚动、嵌套列表与代码块交互](0028-bidirectional-sync-scroll-nested-lists-and-codeblock-ux.md) | 已接受 (Accepted) | 100ms 回声抑制双向同步滚动、递归嵌套列表多层富文本与代码块横向滚动复制 |
| **0029** | [GPUI 0.2 窗口装饰与图层变换约束](0029-gpui-window-chrome-and-overlay-constraints.md) | 已接受 (Accepted) | 红绿灯静态位置 + 折叠安全区；拖拽区显式 `start_window_move()`；灯箱缩放/平移用尺寸与偏移 |
| **0030** | [macOS 研发工具链分工、Instruments 性能诊断与零成本 Ad-Hoc 签名](0030-macos-toolchain-profiling-and-adhoc-signing.md) | 已接受 (Accepted) | 工具链轻重解耦；Instruments 一键追踪掉帧；打包脚本集成零费用本地 Ad-Hoc 签名 |
| **0031** | [侧栏中心导航、无 Tab 头部与多根工作区](0031-sidebar-centric-navigation-and-multi-root-workspace.md) | 已接受 (Accepted) | A/B 全部落地：无 Tab、多根工作区、外部文件组、共享扫描预算、多语言侧栏与错误反馈 |

---

## 二、 作废与合并映射表 (Superseded & Merged)

以下早期探索或中间态决策已被后续架构完全覆盖或推翻，物理文件已精简删除，历史关系留存如下：

| 原编号 | 原文档主题 | 去向 / 替代决策 | 作废与合并原因 |
| :--- | :--- | :--- | :--- |
| **0008** | 手绘 Mermaid 与旧图标 | → **0015** (图标) / **0019** (图表) / **0021** (大纲) | 手绘图表已被 `rusty-mermaid` 废除，图标被 `Assets.car` 替代 |
| **0010** | 模态设置弹窗与旧图标 | → **0013** (分段控件/自动保存) / **0015** (图标) | 模态弹窗被独立窗口取代，有效的分段控件与防抖保存并入 0013 |
| **0011** | 悬浮圆角设置卡片 | → **0013** (跨窗口同步) | 悬浮卡片层级被淘汰，跨窗口同步机制并入 0013 |
| **0012** | 侧边栏设置抽屉 | → **0013** | 抽屉方案被用户明确拒绝，全篇决策被 0013 否定 |
| **0014** | 位图手绘 Squircle 图标 | → **0015** | 位图预裁切方案被 Apple 官方 Icon Composer 架构彻底推翻 |
| **0016** | 手绘拓扑图与全宽撑满 | → **0019** (图表) / **0026** (版心) / **0021** (主题) | 手绘拓扑被纯 Rust 图表替代，版心改用舒适居中，深色修复并入 0021 |
| **0017** | 实验性图表与 StyledText | → **0019** (图表) / **0026** (排版) | `mermaid-rs-renderer` 方案被废弃，StyledText 成果由图表/排版篇继承 |
| **0018** | WebKit + JS 图表管线 | → **0019** | 外部 WebKit 进程与 JS 资产已被完全删除，纯 Rust 引擎胜出 |
| **0023** | 源码编辑器长行强制折行 | → **0024** | 强制折行导致垂直堆叠 bug，已被 0024 的 `whitespace_nowrap` + 横向滚动否定 |
| **0025** | 阅读排版与代码着色 | → **0026** (已落地无界画布) / **0028** (嵌套列表) | 副顶栏移除与嵌套列表已落地，未实施的高亮着色剥离避免误导 |
