# ADR 0022: 图表纹理预算控制与有界 LRU 缓存内存优化

## 状态
已接受 (Accepted)

## 上下文 (Context)
1. **现象分析**：
   - 用户在打开包含多个复杂 Mermaid 图表（如时序图、复杂拓扑流程图）的 Markdown 文档并多开几个文件后，Activity Monitor 显示 GlideMark 物理内存占用（Physical Footprint）陡增至 762MB。
   - 用户指出：“我开了几个文件，直接飙掉 700+占用，这是为什么？是否有方法进行优化？rust 不应该这样搞占用”。
2. **底层内存根因定位 (Root Cause Analysis)**：
   通过 `vmmap`、`heap`、`footprint` 以及 LLDB 对正在运行的进程深入剖析，762MB 占用的完整构成与分配如下：
   - **根因 1：超大 SVG 的未压缩 Retina 位图爆破 (370MB+ 占用)**：
     - `rusty-mermaid` 渲染的复杂时序图与流程图自带巨大绝对宽高，例如 `场景 A 正常退款流` 图表的 SVG 属性为 `viewBox="0 0 3602.72 1539.60" width="3602.72" height="1539.60"`。
     - GPUI 渲染引擎在将 SVG 转化为可供 GPU 采样的纹理时，读取了 SVG 的 `width` 和 `height`，并在 macOS 2x Retina 视网膜屏上以 2 倍物理分辨率进行光栅化（`7205 × 3079` 像素）。
     - 该单个图表的未压缩 32 位 RGBA 位图在 CPU Heap 中占据 `86.6 MB`（`Malloc Large` 中的 86672KB 节点），并同时在 Metal VRAM 中占用对等的 `86.6 MB`（`IOAccelerator (graphics)`）。
     - 7 个不同图表累计生成了 84.6MB、39.0MB、29.3MB、15.8MB、9.8MB 等多个巨型位图，CPU 堆内存占用达 209MB，GPU Metal 显存达 172MB，仅图表位图即占去约 **400 MB**！而此时预览区域的阅读栏宽仅约 800px，图表被硬生生缩放展示，相当于用 7200px 的海报级像素去填充 800px 的屏幕列。
   - **根因 2：GPUI 默认无界图片缓存 (RetainAllImageCache)**：
     - GPUI 框架默认对 `img(path)` 使用 `RetainAllImageCache`，该缓存不会自动清理或逐出旧图片。所有打开过的文档、切换过的 Tab 中光栅化完成的超大图表纹理永久驻留在系统内存与 Metal 显存中，即便文档切换也无法释放。
   - **根因 3：系统文件对话框原生 XPC 伴生进程 (约 58MB)**：
     - 用户通过 `rfd` 调起原生文件/目录选择器时，macOS AppKit 系统会自动派生 `Open and Save Panel Service` (35MB)、`AutoFill` (11MB)、`QLPreviewGenerationExtension` (6MB) 等 XPC 服务。这是 macOS 系统的沙盒辅助进程，与主程序内存独立，但在活动监视器中属于 GlideMark 树状子项。
   - **根因 4：系统 Malloc 缓存页未立即归还操作系统 (约 150MB)**：
     - `vmmap` 显示 `Malloc Large (empty)` 达 146MB，属于 jemalloc/malloc 空闲堆碎片尚未交回 OS 的虚拟脏页。
3. **优化目标与收益预期**：
   - 将 SVG 物理光栅化尺寸限制在合理视口预算（`Diagram Texture Budget`，例如最大宽度 1000px，高度按原有比例自适应，保留完整清晰度的 `viewBox`），单个图表位图将从 `88.7 MB` 直降至 `~2.5 MB`，**单图节约 97% 内存**。
   - 7 个图表总内存从 370MB 降至 15MB 以内。
   - 引入 `SimpleLruCacheProvider`（最多保留 8 个活跃图表纹理），在 Tab 切换或超量渲染时主动通过 `cx.drop_image` 释放 Metal 显存。
   - 整机内存将从 760MB 回落至 **120MB~150MB** 的超轻量水平。

## 决策 (Decision)
1. **实施方案 A（推荐）**：
   - 在 [`mermaid.rs`](crates/glidemark_core/src/mermaid.rs) 的 `normalize_svg_bounds` 中，对 `<svg ...>` 的 `width` 与 `height` 强制应用视口最大宽度约束（1000px），保留矢量 `viewBox` 与自适应纵横比，遏制无节制的高分辨率纹理光栅化。
   - 在 [`preview.rs`](crates/glidemark_app/src/views/preview.rs) 中为预览区域配置有界 LRU 缓存（上限 8 张），释放过往不可见图表的 GPU 纹理。
   - 在 [`state.rs`](crates/glidemark_app/src/state.rs) 中对 `scan_markdown_files` 设置目录扫描上限保护（最多 100 个文件），防止超大项目造成元素树内存溢出。

## 结果 (Consequences)
- 彻底解决打开多图表文档时内存飙升至 700MB+ 的问题，单图光栅化内存缩减 97% 以上。
- 矢量缩放质量不受影响（`viewBox` 保持完整拓扑与文字矢量）。
- 切换多文件或长时间编辑时，有界 LRU 持续自动回收显存，内存曲线平稳。
