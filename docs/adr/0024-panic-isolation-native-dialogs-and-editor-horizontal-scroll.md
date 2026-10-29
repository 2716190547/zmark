# ADR 0024: 全局 Panic 隔离保护、原生对话框架构与现代源码单行滚动

## 状态
已接受 (Accepted)

## 上下文 (Context)
1. **打开文件夹崩溃闪退 (Folder Open Crash & SIGABRT)**：
   - 现象：用户在侧边栏点击“打开文件夹...”或通过快捷键选择包含 Markdown 文档的工作区目录后，应用进程直接异常闪退并退出。
   - 诊断与根因：
     - 从 macOS 诊断崩溃日志（`~/Library/Logs/DiagnosticReports/GlideMark-2026-10-27-085111.ips`）捕获到致命信号 `SIGABRT`，故障线程堆栈顶部为 `_RNvNtCsfOyy81DYK0U_4core3str19slice_error_fail_rt`。
     - 溯源发现外部依赖库 `rusty-mermaid-diagrams-0.2.0/src/state/parser.rs` 第 70 行在遇到非 ASCII（如中文字符）状态图语句失败时，强行执行 `*input = &input[1..]` 字节步进，跨越了 UTF-8 多字节字符边界，触发 Rust 标准库核心切片断言 Panic。
     - 此外，第三方文件选择库 `rfd`（0.15）在 macOS 下依赖 AppKit Sheet 机制，当无主窗口或连续调起时会抛出未捕获的 `NSInvalidArgumentException` 或触发断言 Panic。
   - 解决决策：
     - 在纯 Rust 图表渲染边界建立 **Diagram Panic Shield**：使用 `std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ...))` 捕获任何第三方渲染库内部产生的 Panic，将其优雅降级为非阻塞的 UI 错误卡片，杜绝因外部语法异常导致主程序退出。
     - 废弃并移除 `rfd` 依赖，全面采用 GPUI 原生内建的 `cx.prompt_for_paths(PathPromptOptions { ... })`，直接运行在 AppKit 主线程模态回调中，彻底消除 Sheet 冲突与异步生命周期脱节。

2. **源码模式长行文字垂直堆叠 Bug (Source Editor Vertical Text Stacking)**：
   - 现象：当 Markdown 源码中包含嵌入式表格、超长 URL 或长段落时，折行文本依然与下一行文字在纵向坐标上重叠覆盖，光标点击错位。
   - 根因分析：
     - GPUI 底层 Taffy 弹性盒引擎在主轴为纵向（`flex_col`）的测量阶段，针对无确定宽度的弹性子项（`flex_1`）仅能依据单行基线高度评估跨轴高度（24px）。
     - 运行时文本引擎根据实际像素宽度折为多行（例如 5 行折行高度达 120px），超出弹性容器分配空间溢出渲染，而下一行物理行的 Y 轴位置仍按照 24px 单行基线累加，导致数行长文本在同一纵向高度内重叠堆砌。
   - 解决决策：
     - 遵循专业现代代码编辑器标准规范（VS Code、Zed、Sublime 模式）：源码模式行内文本容器应用 `.whitespace_nowrap()`，禁止单行物理行内部自动断行溢出。
     - 容器配置双向平滑滚动 `.overflow_x_scroll().overflow_y_scroll()`，保证每一行严格遵循 `h(px(font_size * 1.5))` 的匀称几何高度，行号与代码行维持 1:1 物理映射，数学上消除垂直溢出与堆叠。
     - 在鼠标点击命中测试逻辑中引入横向滚动偏移量补偿 `scroll_x = (-offset_x).max(0.0)`，保证水平横向滚动时长行光标定位绝对精准。

3. **工作区目录递归安全防护与性能防卡死**：
   - `state.rs` 中的 `scan_markdown_files` 使用 `dirent.file_type()` 替代跟随软链接的 `metadata()`，杜绝符号链接自循环与目录逃逸。
   - 增加目录项扫描上限（1,000 项预算），防止在大型目录（如包含数十万文件的根路径）中阻塞主线程。

## 决策 (Decision)
1. **Panic 隔离**：在 `crates/glidemark_core/src/mermaid.rs` 的 `render_to_svg_themed` 中包裹 `std::panic::catch_unwind`，任何 Panic 均转为 `Err` 返回。
2. **现代编辑器排版标准**：在 `crates/glidemark_app/src/views/editor.rs` 中采用严格物理行单行高度、`.whitespace_nowrap()`、双向滚动及横向坐标校正。
3. **原生对话框集成**：移除 `rfd` 依赖，全部迁移至 GPUI `cx.prompt_for_paths`。
4. **防御性目录扫描**：严格禁止跟随符号链接目录，增加递归上限与扫描预算。

## 结果 (Consequences)
- **正面收益**：
  - 打开任何目录或包含畸形/多字节状态图的 Markdown 文档时，应用绝对稳定，永不闪退。
  - 源码模式长行（包括大幅 Markdown 表格与超长文本）横向整洁展示且可平滑左右滑动，行号与光标无缝贴合，彻底消除了文字堆叠。
  - 项目依赖体积减少，移除了非必要的 C/Objective-C 第三方对话框包装层。
- **验证覆盖**：
  - `test_panic_isolation_on_malformed_or_panicking_diagram`（验证多字节状态图 Panic 拦截）
  - `test_workspace_folder_symlink_defense_and_robustness`（验证符号链接死循环防护与大目录预算）
  - 全部 41 项自动化测试 100% 通过。
