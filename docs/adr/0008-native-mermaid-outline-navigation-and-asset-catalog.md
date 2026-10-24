# ADR 0008: 原生 Mermaid 拓扑卡片、大纲导航、图标资产目录升级与行内排版修复

## 状态
已接受 (Accepted)

## 上下文 (Context)
在用户实际使用中，提出了以下核心需求与痛点：
1. **图表未渲染**：包含 Mermaid（如 `flowchart TD`）的文档仅渲染为原始文本块，无法直观查看流程图与架构图；
2. **代码块缺乏适配**：缺少一键复制代码、行号与语言卡片；
3. **缺乏文档大纲定位**：长篇文档浏览时无法快速按标题层级跳转；
4. **macOS Dock 栏图标双层嵌套 (一层套一层)**：macOS 26 Tahoe / Sonoma 对仅包含传统 `.icns` 文件的应用强加系统灰色圆角毛玻璃衬底（Squircle Jail）；
5. **行内标记文字被垂直拉高**：表格或多行文本中的行内代码（`InlineItem::Code`）因弹性盒模型默认的 `align-items: stretch` 被异常拉伸为高矩形；
6. **视觉与国际化**：图标线条过细硬冷不够圆润，默认主题需要重命名为“默认”，色彩呼应折叠 Z Logo（暖陶珊瑚色 `#D9532F`），且 8 套主题名称需要全语言翻译。

## 架构决策 (Decisions)

1. **原生 GPU Mermaid 流程拓扑卡片**:
   - 不引入重型 WebKit/Chromium 依赖，保持微内核与微秒级渲染性能。
   - 在 AST 解析层识别 `lang == "mermaid"`，解析其节点定义、连接箭头与类别样式。
   - 在预览视图中渲染为高质感的原生卡片流拓扑，支持双重视图：拓扑图表视图与原始 Mermaid 源码视图的一键平滑切换。
2. **侧边栏双 Tab 结构与大纲定位 (Outline TOC)**:
   - 在 Drawer 顶部引入“文件列表”与“文章大纲”双胶囊分段切换。
   - 解析 AST 所有的 `BlockElement::Heading`，提取 H1~H6 层级缩进，点击即可精准定位至正文对应行并平滑滚动。
3. **修复行内排版拉高 Bug**:
   - 在 `preview.rs` 中的行内代码元素增加 `.h_fit().self_start()` 约束，并在段落与表格的 flex 容器中设置 `.items_center()` / `.items_baseline()`，杜绝跨轴拉伸。
4. **消除 macOS Dock 栏图标双层嵌套 (Squircle Jail) 与资产目录升级**:
   - 彻底移除图标源图中的手绘烘焙投影（Baked Drop Shadow），将画布严格对齐 Apple 官方标准 macOS 连续曲率网格（1024x1024 画布，864x864 纯白圆角矩形，80px 边缘留白，无外层杂色渐变），交由系统物理渲染引擎添加自然阴影。
   - 使用系统内置 `/usr/bin/actool` 将全套 Retina 图标资产打包为标准的 `Assets.car`。
   - 在 `Info.plist` 中声明 `CFBundleIconName = "AppIcon"` 与 `CFBundleIconFile = "AppIcon"`。
   - 打包脚本自动调用 `/System/Library/Frameworks/CoreServices.framework/.../lsregister -f` 强制刷新 LaunchServices，并清空 `/private/var/folders/.../com.apple.dock.iconcache` 缓存，彻底根除 macOS 26 Tahoe / Sonoma 强加的毛玻璃灰色圆角外框。
5. **官方标准矢量图标库 (Lucide SVG via AssetSource)**:
   - 杜绝使用手绘 `div` 拼接图标。通过 GPUI 内置原生 `AssetSource` 特征机制加载官方标准 Lucide 矢量图标（`panel-left.svg`, `folder.svg`, `columns-2.svg`, `book-open.svg`, `code.svg`, `download.svg`, `save.svg`, `settings.svg`, `copy.svg`, `files.svg`, `list.svg`）。
   - 通过 GPUI 原生 `svg().path("...").size(...).text_color(color)` 渲染，色彩动态跟随主题，Retina 屏绝对平滑，大幅削减 150+ 行手工拼凑代码。
6. **视觉设计升级与全量国际化**:
   - 默认主题命名为“默认 (Default)”，强调色设定为暖陶珊瑚色 `#D9532F`，完美呼应折叠 Z Logo；
   - 8 套调色板全量支持 10 国语言本地化翻译，清理掉未使用的测试主题代码。

## 影响 (Consequences)
- 彻底解决长文档排版与图表浏览需求；
- 用户在 macOS 启动台与 Dock 栏获得 100% 官方第一方质感的精美无底框图标；
- 全平台性能依然保持 6.6MB 原生编译极速响应。
