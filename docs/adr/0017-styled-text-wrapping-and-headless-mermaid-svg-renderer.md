# ADR 0017: 引入 GPUI 原生 StyledText 排版引擎与纯 Rust Mermaid 矢量 SVG 渲染管线

## 状态
已接受 (Accepted) - 替代 (Supersedes) ADR 0016 中关于手绘图表卡片与 Flex 包装分词方案

## 上下文 (Context)
1. **排版溢出与截断的深层根因**：
   - 此前预览区将 Markdown 行内元素（文字、加粗、斜体、代码、链接）解析为多个独立的 `div` 元素放置于 `.flex().flex_wrap()` 容器中。
   - 在 GPUI（Taffy 布局引擎）中，Flex 子项的最小宽度默认为 `min-width: auto`（即内容无断行时的最大宽度）。对于无空格分割的连续中文长段落，每个子 `div` 会直接要求等同于整段文字长度的宽幅，导致 Flexbox 无法在字符级别折行，而是作为一个刚性矩形直接超出屏幕右边界被视口裁切。
2. **手绘流程图卡片缺陷与视觉严肃性**：
   - 此前通过手写 GPUI 卡片、Unicode 箭头（`▼`）及分支徽标模拟 Mermaid 流程图，不仅无法真实还原复杂的拓扑分支、判断菱形与数据库圆柱体，还引入了非正式的符号修饰，违背了严肃排版与原生 macOS 软件质感。
3. **开源生态对照与工程选型**：
   - 逆向参考 Zed 编辑器与现代化桌面 Markdown 实现（如 `merman`、`mermaid-rs-renderer`），工业级预览方案均采用成熟的无浏览器（Headless）纯 Rust 渲染管线，将 Mermaid 代码直接编译为标准的 W3C 矢量 SVG，再交由 GPUI 内置的 `usvg` 图形管线进行 GPU 视网膜抗锯齿绘制。

## 决策 (Decision)
1. **全面转向 GPUI 原生 `StyledText` 流式排版引擎**：
   - 彻底废除 `.flex().flex_wrap()` 拼接 `div` 的行内元素排版方案。
   - 段落与标题采用单一 `StyledText`，利用 `with_highlights` 将加粗（`FontWeight::BOLD`）、斜体（`FontStyle::Italic`）、行内代码（`background_color` + `color`）和链接（`underline`）作为非重叠样式区间（`HighlightStyle`）统一传入。
   - 依托 GPUI 文本子系统底层的 `LineWrapper`，在容器像素边界处自动对中英文、CJK 字符与拉丁词汇进行自然换行，同时配合所有外层容器的 `.w_full().min_w(px(0.))` 约束，彻底根除横向撑破与截断。
2. **集成纯 Rust 无头图表渲染库 (`mermaid-rs-renderer`)**：
   - 引入 `mermaid-rs-renderer = { version = "0.3.1", default-features = false }`，不依赖 Node.js、Chromium 或 Puppeteer。
   - 在 [`crates/glidemark_core/src/mermaid.rs`](file:///Users/apple/Documents/工作区/GlideMark/crates/glidemark_core/src/mermaid.rs) 提供 `render_to_svg_themed(code, is_dark)`，支持亮色模式（Modern Theme）与深色模式（Dark Theme）智能适配。
   - 渲染出的标准 SVG 写入临时缓存，并通过 GPUI 原生 `img(path).max_w_full()` 呈现为高保真无损矢量图。
3. **视觉符号与图标规范化 (macOS HIG)**：
   - 清理所有非标准占位符与颜文字装饰，图片占位采用 2.25pt 描边的原生矢量 `image.svg`，任务清单复选框采用矢量 `check.svg`，列表项目符号采用标准的半角破折线。

## 结果 (Consequences)
- 中英文段落无论窗口如何缩放或处于何种分屏比例，均能严格自适应充满宽度并精准折行。
- Mermaid 图表获得 100% 官方级别的标准矢量连线、箭头、菱形判断与圆柱体数据库等图形表达，彻底消除简陋的卡片与符号拼接感。
- 代码库精简了 300 多行自定义卡片逻辑，遵循 Ponytail ULTRA 模式的“删除优于堆砌”。
