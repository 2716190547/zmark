# ADR 0019: 采用 100% 纯 Rust `rusty-mermaid` 替换 WebKit / JavaScript 外部渲染管线

## 状态
已接受 (Accepted) - 替代 (Supersedes) ADR 0018 中关于 macOS 原生 WebKit 外部进程与 JS 资产的决策

## 上下文 (Context)
1. 在 ADR 0018 中，为解决实验性库 `mermaid-rs-renderer` 走线折返遮挡标题与 `merman` 语法报错问题，曾临时引入 macOS 系统无头 WebKit + `mermaid.min.js` 管道。
2. 尽管 WebKit 方案在视觉上达到了官方 Mermaid.js 标准，但引入了外部跨进程通信、3600 行外部 JS 资产依赖以及平台绑定（macOS 专有 Swift 辅助脚本），违背了 GlideMark 作为高性能单二进制桌面工具的极简原则。
3. 经全面调研与实机测试 GitHub / crates.io 开源纯 Rust 项目：
   - `base58ed/rusty-mermaid` (v0.2.0)：内置分层图布局引擎 `rusty-mermaid-dagre`，原生支持复合子图（compound subgraphs）、跨子图平滑贝塞尔走线、圆柱体存储节点 `[(...)]` 以及 `Theme::light()` / `Theme::dark()`。经实测，用户多层嵌套子图（前端交互层、后端服务层、存储与模型底座）与双向跨层连线（`F <--> S1`, `F <--> S3`）全部精准排版，未发生连线折返穿插标题。
   - `mermaid-little` (v11.14.0-6)：多子图复合排版存在坐标重叠 Bug（S1 与 S3 放置在相同坐标 `(8, 8)`），暂未稳定。
   - `mermaid-rs-renderer` (v0.3.1)：多子图走线发生 180° 折返切割标题。
   - `Latias94/merman` (v0.9.2)：对未加引号结构字符直接 Panic，且依赖庞大。

## 决策 (Decision)
1. **采用 `rusty-mermaid` (v0.2.0) 作为主矢量图表渲染引擎**：
   - 启用 `rusty-mermaid` 的 `svg` 特性，在 `glidemark_core` 中直接执行 Mermaid 代码解析与 SVG 矢量生成。
   - 增加轻量预处理器：自动规范化 `subgraph ID["Title"]` 中多余的引号，确保标题与节点标签干净呈现。
   - 对接 `Theme::light()` 与 `Theme::dark()`，与 GlideMark 主题色板无缝联动。
2. **彻底移除 WebKit 外部辅助工具与 JavaScript 资产**：
   - 彻底删除 `crates/glidemark_app/assets/mermaid.min.js`（3636 行代码）。
   - 彻底删除 `crates/glidemark_app/assets/helpers/glidemark_mermaid.swift`。
   - 移除 `glidemark_core` 中的 WebKit 进程生成逻辑，消除子进程启动开销。
3. **保留并延续 ADR 0018 的内存级单例缓存 (`DIAGRAM_CACHE`)**：
   - 保持 `(hash(code), is_dark)` 全局缓存机制不变，视口滚动期间 0 次重复解析，确保 120 FPS 高刷流畅体验与稳定低内存占用。

## 结果 (Consequences)
- 达成 100% 纯 Rust 原生渲染，零外部进程、零 JS 脚本、零 WebKit 框架依赖。
- 架构图、子图与跨层连接排版精准优雅，彻底杜绝走线折返穿插文字问题。
- 代码库与应用体积大幅削减（减少 3600+ 行静态 JS 资产与跨进程调度代码）。
