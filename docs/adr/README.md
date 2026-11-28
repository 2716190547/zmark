# ZMark 架构决策记录 (ADR 索引)

本文档是 ZMark 架构演进与技术选型的唯一权威索引表。历史 ADR 保留决策时的产品名称与源码路径，当前实现使用 `zmark_core` / `zmark_app`。
所有技术决策遵循轻量、务实、直面根因原则，早期已被替代的中间态探索已并入下表所列去向，不保留冗余文件。

---

## 一、 有效架构决策清单

| 编号 | 标题 | 状态 | 核心决策与结论 |
| :--- | :--- | :--- | :--- |
| **0001** | [核心技术栈与基础架构](0001-core-tech-stack-and-architecture.md) | 已接受 (Accepted) | 采用 Rust + GPUI 渲染引擎与自研纯文本编辑核心 |
| **0002** | [虚拟化块渲染与同步滚动架构](0002-virtualized-block-rendering-and-sync-scrolling.md) | 已接受 (Accepted) | 确立源码锚点与分块渲染模型；双向同步在 0028 落地 |
| **0003** | [自包含 HTML 独立单文件导出](0003-self-contained-html-export.md) | 已接受 (Accepted) | 导出内联完整 CSS 样式的单文件离线 HTML，无需依赖外部浏览器 |
| **0004** | [Core 与 App 双 Crate 解耦边界](0004-core-ui-crate-decoupling.md) | 已接受 (Accepted) | 逻辑解析（zmark_core）与 GPU 渲染层（zmark_app）严格隔离 |
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
| **0031** | [侧栏中心导航、无 Tab 头部与多根工作区](0031-sidebar-centric-navigation-and-multi-root-workspace.md) | 已接受 (Accepted) | A/B 全部落地：默认无 Tab（0040 增加可选标签条）、多根工作区、外部文件组、共享扫描预算、多语言侧栏与错误反馈 |
| **0032** | [视觉层级精进、无感知全屏适配与 HiDPI 流程图重采样灯箱](0032-canvas-header-visual-refinement-and-hidpi-lightbox.md) | 已通过 (Accepted) | 全屏左边距动态感知、侧栏草稿去常驻化、头部大标题与视图模式平铺统一、HiDPI 矢量重采样与显存保护 |
| **0033** | [现代化设置视窗、侧栏层级降噪、灯箱纸张画布与沉浸阅读排版](0033-modern-settings-drawer-refinement-and-editor-reading.md) | 已通过 (Accepted) | 灯箱纸张实体画布隔离透光、侧栏移除工作区标头与灰点并升级13px、设置窗口无框胶囊重构、编辑器对齐/1.6x行高/轻量语法高亮 |
| **0034** | [纯净中性调色体系、Apple 原生分组设置与均高切分软折行](0034-miaoyan-palette-apple-settings-and-visual-line-wrap.md) | 已通过 (Accepted) | 妙言纯净中性调色重构消灭脏感、去除未保存红点、侧栏两级层级/文件夹状态开合/Hover新建、18px顶栏大标题、Apple Grouped Inset Cards设置卡片、均高视觉行切分软折行与多色语义高亮 |
| **0035** | [无缝贯穿设置视窗、系统化调色推导、图标线宽归一、点部语义上色与 Mermaid 语法脱糖](0035-settings-layout-derived-palette-icon-stroke-and-mermaid-desugaring.md) | 已通过 (Accepted) | 彻底移除 44px 横栏实现全高无缝分栏；macOS 原生 Segmented Control 槽底浮雕调色推导；顶栏设置常亮修复与 16px 图标线宽 1.8~2.25 viewBox 光学归一；Markdown 列表/语法标缀点部专属色；Mermaid 复合连线 & 自动笛卡尔展开与容错 |
| **0036** | [选中态语义 Token、代码墨色、灯箱位图通道修复与设置页扁平化](0036-selection-tokens-code-ink-lightbox-bitmap-and-settings-flattening.md) | 已通过 (Accepted) | Phase G 历史决策；选中色、标题对齐及侧栏搜索推迟项由 0037 局部替代，其余继续有效 |
| **0037** | [真实文字布局、虚拟化预览选区与侧栏文件名搜索](0037-real-text-layout-and-virtualized-preview-selection.md) | 已通过 (Accepted，含 P2 限制) | 真实折行与行级复用、预览跨块字符选区及纯文本复制、系统输入与后台文件名搜索；模式相关标题对齐、默认/Claude 暖选中色；遗留见 Phase H 验收 |
| **0038** | [发行资产迁回源码仓库，并通过过渡版归档 zmark-releases](0038-move-releases-into-the-source-repository.md) | 已通过 (Accepted) | 安装包、签名更新清单与发布说明从 0.2.13 起发布到源码仓库 Releases，旧发行仓库归档保留 |
| **0039** | [预览同帧测量与按文档保存的编辑、预览状态](0039-same-frame-preview-measurement-and-per-document-state.md) | 已通过 (Accepted) | 可见块在 prepaint 中先测量再定位，消除按估算高度排版的一帧；撤销历史、预览高度与滚动位置按文档保存；图片与 Mermaid 尺寸记忆 |
| **0040** | [可选标签条](0040-optional-tab-bar.md) | 已通过 (Accepted) | 修订 0031 第 1 节：默认仍无标签，设置开关开启后头部显示由 `open_docs` 派生的标签条；拖动排序写回会话顺序；`Ctrl+Tab` 与按位置跳转快捷键常开 |
| **0041** | [预览按块编辑](0041-preview-block-editing.md) | 已通过 (Accepted) | 开关开启后单击预览块，就地换成编辑共享 `EditorEngine` 的源码框；编辑期间不重新解析，退出时解析一次；代码块也可编辑 |

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
