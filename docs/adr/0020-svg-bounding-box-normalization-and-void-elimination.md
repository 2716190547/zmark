# ADR 0020: 纯 Rust 图表 SVG 视口自适应几何归一化 (ViewBox Bounding Box Normalization)

## 状态
已接受 (Accepted)

## 上下文 (Context)
1. **复合子图顶部巨大空白根因**：
   - 用户在预览包含横向排版与纵向子图（如 `flowchart LR` 内嵌多个 `subgraph ... direction TB`）的复杂架构图时，图表上方出现了大面积空白，图表内容被挤压至卡片最底部（如图表实际高度仅 255px，但卡片占位高达 1349px）。
   - 经深入排查 `rusty-mermaid-dagre` 布局管线与 `bridge.rs`：
     - 在提取内嵌子图时，`extract_directed_subgraphs` 解除了子图内 9 个叶子节点的父级关联，但未从外层主图中移除这些孤立节点。
     - 外层 Dagre 布局执行时，将这些游离孤立节点分配到 `y = 0..1000` 的层级中，迫使外层子图节点被推移至 `y = 1181`。
     - 随后的 `apply_subgraph_positions` 将 9 个节点重新归位至各自子图（`y = 1181` 附近），但未重新对整个图表执行垂直平移（Shift up to `y = 0`）。
     - 最终生成的 SVG 包含 `viewBox="0 0 1494.28 1349.02"`，其实际内容仅分布在 `y >= 1053.82`，导致顶部遗留多达 1053px 的纯空白几何空洞。
2. **影响**：
   - GPUI 将 1349px 视口等比例缩放至预览区宽度，导致图表元素极度扁平且顶部留白严重失调。

## 决策 (Decision)
1. **在 `glidemark_core` 输出层引入零外部依赖的矢量视口归一化算法 (`normalize_svg_bounds`)**：
   - 解析 SVG 内实际图元盒模型（遍历非百分比 `<rect>` 与 `<text>` 坐标，提取 `min_x, min_y, max_x, max_y`）。
   - 将原点偏移量 `(padding - min_x, padding - min_y)` 动态注入至最外层 `<g transform="...">`，无需逐一改写内部数百个贝塞尔曲线控制点。
   - 依据内容真实几何包围盒重写 `<svg viewBox="0 0 new_w new_h" width="new_w" height="new_h">`，确保四周均匀留白 20px。
   - 针对暗黑模式背景 `<rect width="100%" height="100%">` 保持完整铺满，色彩无缝联动。

## 结果 (Consequences)
- 图表高度从 1349px 紧凑收敛至 295px（减少 78% 的冗余垂直留白），上下左右居中且四周均匀留白 20px。
- 所有复合嵌套流程图自适应紧凑排版，无论内部分层算法是否存在全局偏移，输出的 SVG 均保持严丝合缝。
- 零性能损耗（微秒级纯字符串解析，不涉及二次重排），测试套件全覆盖。
