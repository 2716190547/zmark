# ADR 0036: 选中态语义 Token、代码墨色、灯箱位图通道修复与设置页扁平化

## 状态
已提议 (Proposed) - 待执行端按工单 G0 ~ G4 实施，审查端终验后改为 Accepted

## 上下文 (Context)
Phase F 交付后，用户提供 4 张标注图（设置页、主窗口两栏、灯箱、妙言参考），提出以下问题：

1. **灯箱颜色与正文不一致**。已定位根因：`workspace.rs::open_lightbox` 调用 `gpui::Image::to_image_data(svg_renderer)`。该函数对 SVG 直接使用 resvg 的 **RGBA 预乘 alpha** 像素，没有做 R/B 通道交换和 alpha 还原（GPUI 自己的 `img()` 加载路径会调用 `swap_rgba_pa_to_bgra`，两条路径不一致）。证据：`#9370db` 紫色边框显示为 `#db7093` 粉色；子图填充 `#ecf2dc99` 按直通 alpha 解读后叠白底约为 `#b4bcb8`，与截图灰绿一致。截图中灯箱内图为浅色变体而窗口为深色，执行端需额外验证 `open_lightbox` 取用的 SVG 与预览区是同一 `is_dark` 变体。
2. **标题与内容区错位**：画布标题在 Source/Split 下被 `canvas_title_offset` 推到 64px（ADR 0033 对齐代码第 1 列），用户要求改为对齐左边，并加大上下留白。
3. **未保存红点**：侧栏草稿行与所有文件行的红点（`theme.accent` 圆点）被判定为噪音。
4. **选中态背景不统一**：13 处使用 `warm_pill_bg`（`#EFF1F4` 偏蓝灰），与妙言暖灰风格不一致，且不随 8 套调色板变化。
5. **代码块**：预览区代码块带外框、`TEXT / N lines` 头部、行号，用户要求简化；无语言块的长行被裁切；源码区围栏代码与正文同色，需区分。
6. **设置页**：搜索框无用；右栏“分组卡片套行内项”多层嵌套；配色体系用圆点表达。
7. **顶栏右侧留白**与左侧不一致。
8. **侧栏列表**：用户希望参考妙言的两行列表项和圆角搜索框。

## 决策 (Decisions)

### 1. 选中态语义 Token（取代 warm_pill_*）
在 `ThemeColors` 新增三个派生方法，并**删除** `warm_pill_bg / warm_pill_hover / warm_pill_text`：

| Token | 定义 |
| :--- | :--- |
| `selection_bg()` | `text_primary.opacity(0.07)`，叠在任意底色上 |
| `hover_bg()` | `text_primary.opacity(0.04)` |
| `selection_text()` | `text_primary` |

- 中性色派生，随 8 套调色板和明暗模式自适应，不引入新的硬编码色值。
- **Solarized 浅色 `text_primary` 由 `#657b83` 调整为 `#586e75`**：原值对其 `bg` 对比度约 4.1:1，低于样张卡可读性门槛 4.5:1（见设置页一节）；这是该门槛的直接后果，不属于范围蔓延。
- 所有侧栏行、设置导航、设置列表项、顶栏胶囊、预览区 Mermaid 放大按钮统一改用。
- 备选“强调色 10% 叠加”被否决：选中行会随调色板变色，与妙言的中性风格不符。

### 2. 代码墨色 `syntax_code`
- `ThemeColors` 新增 `syntax_code: Hsla`，8 套调色板明暗共 16 色，沿用 F4 `syntax_bullet` 的做法。默认调色板取妙言橄榄金色相；浅色模式对 `bg` 对比度 ≥ 4.5:1，深色同样。
- **源码区**：围栏代码行（`compute_fenced_code_lines` 命中的行，含围栏标记行）文字使用 `syntax_code`；围栏标记行保持 `text_muted` 以弱化。
- **预览区**：代码块文字使用 `syntax_code`；**不做词法着色**（YAGNI）。

### 3. 预览区代码块简化
- 去掉外框（`border_1`）、头部栏（语言徽标、`N lines`）和行号列。
- 保留 `code_bg` 圆角底色；复制按钮改为鼠标悬停代码块时才在右上角出现。
- **换行规则**：有语言标记且非 `text` 的块自动换行；无语言或 `text` 块保留 `overflow_x_scroll` + `whitespace_nowrap`（多为需要对齐的 ASCII 目录树，换行会破坏竖线对齐）。
- Mermaid 块不受影响。
- 判定函数 `should_wrap_code_block(lang) -> bool` 为纯函数：`Some(l)` 去首尾空白并转小写后，**非空且不等于 `text`** 返回 `true`（换行）；`None`、空串、`text` 返回 `false`（横向滚动）。

### 4. 灯箱位图通道修复
- `open_lightbox` 不再调用 `Image::to_image_data`。改为 `gpui::Image::from_bytes(ImageFormat::Svg, ..)`，灯箱以 `img(ImageSource::Image(arc))` 加载，走 GPUI 的 `swap_rgba_pa_to_bgra` 路径。
- `LightboxState` 中保存 `Arc<gpui::Image>` 取代 `Arc<RenderImage>`。
- **释放必须两步**（G 阶段审查发现并经 GPUI 0.2.2 源码核实）：`Image::remove_asset(cx)` 只清除 `App.loading_assets` 缓存，**不会**从 sprite atlas 移除 GPU 纹理，唯一的纹理移除点是 `Window::drop_image` / `App::drop_image(RenderImage)`。因此统一经一个释放函数处理：先在缓存仍存在时用 `Arc<Image>::get_render_image(window, cx)`（不触发加载）取回 `Arc<RenderImage>`，调用 `cx.drop_image(render, window)`，再调用 `remove_asset(cx)`。关闭、文档切换、`open` 替换三条路径都必须走该函数。释放语义保持 C2b：同一时刻最多一张 HiDPI 位图。
- 灯箱卡片底色统一为 `theme.surface`（去掉浅色模式下的 `rgb(0xffffff)`），与预览区容器一致。
- 备选“自己调 `render_pixmap` 后手动交换通道”被否决：需新增 `image`、`smallvec` 两个直接依赖，且重复实现 GPUI 内部逻辑。
- 约束：先写复现测试（`hidpi_svg` 的 `is_dark` 与预览一致），再改实现。

### 5. 顶栏与画布
- 画布标题左缘对齐内容区 16px 外边距（Source / Split / Preview 一致）。`canvas_title_offset` 在三种视图下均返回 0.0 时直接删除该函数及其调用；ADR 0033 中“标题对齐代码第 1 列”的约定由本条取代。侧栏折叠时的 76px 红绿灯安全区与展开按钮不变。
- 画布顶栏垂直内边距各增加 4px（总高约增加 8px）。
- 顶栏最右侧图标距窗口边缘的留白等于左侧侧栏头部的水平内边距（执行端测量后取同一数值，在报告中给出）。
- 删除侧栏所有未保存红点（草稿行、普通文件行、已打开文件行）。**顶栏保存按钮**在文档有未保存修改时使用强调色，其余时间为静音灰。草稿关闭时的保存确认弹窗保留。`is_active_dirty` 参数若仅用于红点，一并删除。

### 6. 设置页
- **删除设置搜索**：`filter_settings_cross_tab`、`SearchMatchResult`、`render_highlighted_text`、`matches_*`、`render_empty_state`、`search_query / is_search_focused` 状态及按键处理、`search_settings / search_no_results` 文案（4 种语言）、相关测试。左栏顶部保留 36px 红绿灯安全区。理由：整页只有约 10 个设置项，且项目没有文本输入基础设施（当前实现手写按键拼接，不支持 IME 输入）。
- **去嵌套**：外观 / 编辑与保存 / 界面语言 / 关于四个标签页统一去掉外层分组卡片；内容撑满右栏，只保留行内圆角项，选中项使用 `selection_bg()`。分段控件整行宽度，不再包卡片。
- **配色体系**：圆点换成圆角矩形样张卡，两列撑满整行。每张卡用 `ThemeColors::resolve(id, 当前明暗)` 的 `bg` 作底，“Aa”使用 `text_primary`，一条强调色短横线使用 `accent`，调色板名称在下方。选中项使用 2px `accent` 描边。

### 7. 侧栏列表项两行样式
- 文件行改为两行：文件名 + 修改日期（`YYYY/MM/DD HH:mm`，来自文件 mtime，扫描时读取并缓存，渲染期不做 IO）。文件夹行保持单行。草稿行日期显示为空行并省略第二行。
- **圆角搜索框推迟**：侧栏当前没有搜索功能，也没有可复用的文本输入控件。本批不做，作为后续独立工单（需要先实现带 IME 支持的输入控件，再供侧栏搜索使用）。

## 实施工单与文件边界
所有子代理遵守 AGENTS.md：同一文件只有一个写入者。

| 工单 | 内容 | 独占文件 |
| :--- | :--- | :--- |
| **G0**（先行，单独提交） | `selection_bg / hover_bg / selection_text`、`syntax_code`（16 色）及其单元测试；`warm_pill_*` 暂保留 | `theme.rs` 及其测试 |
| **G1** | 设置页：删搜索、去嵌套、样张卡、改用新 token、i18n 清理 | `views/settings.rs`、`i18n.rs`、`tests/settings_*` |
| **G2** | 侧栏：删红点、两行列表项（含 mtime 缓存）、改用新 token | `views/drawer.rs`、`state.rs`（仅 mtime 字段）、`tests/sidebar_*` |
| **G3** | 代码块简化与换行规则、预览/源码使用 `syntax_code`、Mermaid 按钮改用新 token | `views/preview.rs`、`views/editor.rs` |
| **G4** | 灯箱通道修复与卡片底色；顶栏标题对齐、垂直留白、右侧留白、保存按钮脏态色；`workspace.rs` 中 `warm_pill` 迁移 | `views/lightbox.rs`、`views/workspace.rs`、`tests/hidpi_lightbox_test.rs`、`tests/header_*` |

G0 完成并提交后，G1 ~ G4 并发。全部合入后由主代理删除 `warm_pill_*`，运行 `cargo test --workspace` 与 `./scripts/bundle_macos.sh`。

## 结果 (Consequences)
- 选中 / 悬停背景由两个 token 统一管理，新增调色板自动适配。
- 灯箱与预览区颜色一致；释放路径为 `get_render_image` → `drop_image` → `remove_asset` 三步，显存释放行为由行为测试（非字符串断言）保证。
- 删除设置搜索后，如将来设置项增多，需要先做通用输入控件再重新引入。
- 侧栏文件行高度由 28px 增加到约 44px，同屏可见文件数减少。
- 删除红点后，自动保存关闭时顶栏保存按钮是唯一的未保存提示。
