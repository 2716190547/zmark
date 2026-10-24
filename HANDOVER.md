# GlideMark 项目交接与工作总结文档 (Handover & Summary)

> **版本**：v0.1.0  
> **日期**：2026-10-24  
> **核心架构**：Rust + GPUI (0.2) + Taffy + Pulldown-cmark  
> **产物位置**：`/Applications/GlideMark.app` (macOS 原生 Universal/arm64 独立应用)

---

## 一、 项目背景与架构定位

针对市面上现有 Markdown 编辑阅读器**内存占用过大（如 Electron 套壳）、滚动与渲染卡顿、视觉过于复杂或 UI 不合心意**的痛点，GlideMark 被设计为一款由 **Rust + GPUI** 强力驱动的单文档 GPU 原生极速阅读与编辑器：
- **微内核设计**：完全解耦的双 crate 架构：
  - `glidemark_core`：纯逻辑层，涵盖 AST 词法解析、双向行块映射、独立离线 HTML 导出、文件监控、配置持久化与 10 国语言字典。不依赖任何 UI 引擎。
  - `glidemark_app`：GPUI 原生视窗层，基于 GPU 渲染树、Taffy flexbox 布局引擎、自研纯文本与行光标引擎、无框化极简顶栏与分屏拖拽。
- **美学标杆**：深度参考**妙言 (MiaoYan)** 极简主义设计语言，以纯白呼吸感画布、极细发丝级分割线与鲜活绿植强调色打造克制、专业、纯粹的书写与阅读体验。

---

## 二、 近期关键工作与深度问题解决

### 1. 正文无法滑动与分屏预览丢失 Bug 深度根治 (Layout & Scrolling)
- **根因分析**：
  - **滚动陷阱 (Flexbox Auto Min-Height)**：在 CSS Flexbox 及 GPUI 底层的 Taffy 布局引擎中，Flex 容器的子元素默认 `min-height: auto`。当正文 Markdown 内容较长时，容器自动延展为内容的内在总高（如 5000px），导致视口计算溢出高度恒为 0（`scroll_max = 0`），滚轮与触控板事件无法触发有效位移。
  - **分屏挤压 (Flexbox Auto Min-Width)**：在左右分屏模式下，左侧源码编辑器的文本行单行可能长达上千像素。未加 `min-width: 0` 限制时，编辑器列会随最长文本行水平无限膨胀，直接将右侧预览列完全挤出可视窗口。
- **工程解决**：
  - 在顶层内容容器、视图模式分支容器、编辑器容器、阅读预览容器及抽屉列表中，严格补充 `.min_h(px(0.))` 与 `.min_w(px(0.))`。
  - 对左右分屏列增加 `.w(relative(...)).min_w(px(0.)).overflow_hidden()` 刚性约束。
  - 彻底恢复了平滑的触控板滑动、双向滚轮滚动，分屏模式下左右两栏各司其职，均能在各自区域内流畅滚动。

### 2. 文件打开直接闪退 Bug 根治 (Thread Safety & Re-entrancy)
- **根因分析**：此前调用 `rfd::FileDialog::pick_file()` 为同步阻塞调用，在主线程触发 AppKit 原生模态循环（`NSOpenPanel runModal`）。模态激活期间系统输入法或按键变更事件被派发至 GPUI，导致 GPUI 内部的 `AppCell` 发生二次可变重入借用（`panic_already_borrowed`），造成程序瞬时崩溃。
- **工程解决**：升级为完全异步非阻塞的 `rfd::AsyncFileDialog::new().pick_file()`，通过 `cx.spawn(async move |this, app| ...)` 安全派发回调，彻底杜绝重入借用。

### 3. 系统级拖拽直接打开文件 (Native Drag & Drop)
- 在窗口根视图注入 `.can_drop(...)` 与 `.on_drop<ExternalPaths>(...)` 监听器。
- 支持直接从访达 (Finder) 拖入 `.md`、`.markdown` 或 `.txt` 文件，应用瞬时完成无感加载与视图刷新。

### 4. 标题双击原地内联重命名 (Inline Rename)
- 顶部标题栏支持鼠标双击进入原地内联编辑状态（呈现强调色微光晕与下划光标）。
- 用户可直接打字输入新文件名：按 `Enter` 立即执行磁盘级 `fs::rename` 并同步内存与路径状态；按 `Esc` 即可取消编辑。

### 5. 顶栏功能图标全面“去框化”与视觉弱化 (Borderless Minimalism)
- **功能图标去框化**：移除此前功能图标外围的硬性 1px 方形边框（`border_1`）。常态下完全无框、透明融入顶栏，鼠标悬停时平滑浮现柔和微胶囊衬底（`theme.surface`），点击时呈现强调色高亮反馈。
- **微发丝分割线**：顶栏与分栏弱化为 1px 超浅半透明微发丝（MiaoYan: `#EDEDED`）。
- **可拖拽分屏调节条**：保留 5px 舒适鼠标拖拽感应区域，视觉中心收敛为 1px 极细线条；支持双击调节条一键回正为 50/50 对称分屏。
- **零颜文字/严肃化**：全应用坚决剔除任何 Emoji 或颜文字符号，所有操作均由高精度原生矢量几何路径（Retina 无损缩放）呈现。

### 6. App 图标重构与 macOS 原生打包安装 (Icon & App Bundle)
- **去除套娃层叠**：彻底删除了此前内部生成的浅黄色小方框嵌套层，遵循现代 macOS 单层 Squircle 规范。
- **质感配色**：纯白底板搭配微妙的自然下沉环境阴影；中央折叠几何 Z 轮廓采用暖色红褐色（`#C24128`）至暖金琥珀色（`#F59E0B`）的 45° 对角线柔和渐变填充。
- **编译发布**：通过 `scripts/generate_icon.py` 经 4x 超采样生成全套 Retina PNG，经 `iconutil` 编译为标准 `AppIcon.icns`。
- **一键打包**：提供 `./scripts/bundle_macos.sh`，生成正规 macOS Bundle 架构（含 `Info.plist` 与 Markdown 文件关联绑定），并自动安装至 `/Applications/GlideMark.app`。

### 7. 主题调色板与 10 国语言国际化系统
- **8 套精心调配的主题配色**：
  - **MiaoYan**（妙言极简纯白，默认）
  - **Claude Warm**（克劳德温润羊皮纸）
  - **macOS Vibrant**（原生系统高对比）
  - **Tokyo Night**（经典暗夜霓虹）
  - **Nord**（北欧冷冽雪山）
  - **GitHub Light**（经典代码白）
  - **Solarized Light**（经典低眩光柔和暖黄）
  - **One Dark Pro**（经典深空灰代码暗色）
- **10 国语言全量覆盖**：简体中文 (`zh-CN`)、繁体中文 (`zh-TW`)、英语 (`en-US`)、日语 (`ja-JP`)、韩语 (`ko-KR`)、德语 (`de-DE`)、法语 (`fr-FR`)、西班牙语 (`es-ES`)、俄语 (`ru-RU`)、葡萄牙语 (`pt-PT`)。

---

## 三、 代码仓库架构与文件目录速查

```text
GlideMark/
├── Cargo.toml                                 # 顶层虚拟工作区配置
├── Cargo.lock                                 # 依赖锁定文件
├── README.md                                  # 项目快速上手文档
├── HANDOVER.md                                # 详细交接与技术总结文档
├── GLOSSARY.md                                # 领域专业术语定义字典
├── docs/
│   └── adr/                                   # 架构决策记录 (ADR)
│       ├── 0001-core-tech-stack-and-architecture.md
│       ├── 0002-virtualized-block-rendering-and-sync-scrolling.md
│       ├── 0003-self-contained-html-export.md
│       ├── 0004-core-ui-crate-decoupling.md
│       ├── 0005-unified-topbar-custom-editor-and-palette-system.md
│       ├── 0006-async-file-dialog-and-external-drop-handling.md
│       └── 0007-miaoyan-palette-borderless-toolbar-and-flex-scroll-fix.md
├── assets/
│   ├── AppIcon.icns                           # 编译后的 macOS 原生图标
│   └── icon.iconset/                          # 16x16 至 512x512@2x 各尺寸位图
├── scripts/
│   ├── bundle_macos.sh                        # 自动化 Release 编译与安装至 /Applications
│   └── generate_icon.py                       # 4x 超采样矢量渐变图标生成脚本
└── crates/
    ├── glidemark_core/                        # 核心纯逻辑层 (独立无 UI 依赖)
    │   ├── src/
    │   │   ├── lib.rs
    │   │   ├── ast.rs                         # Markdown 语法树与行映射
    │   │   ├── sync_scroll.rs                 # 源码与预览双向同步坐标映射
    │   │   ├── document.rs                    # 文档状态机与外部文件监控
    │   │   ├── export.rs                      # 自包含单文件独立 HTML 离线导出
    │   │   ├── config.rs                      # 偏好配置模型 (TOML 序列化)
    │   │   └── i18n.rs                        # 10 国语言完整文案字典
    │   └── tests/                             # 核心层自动化单元测试
    └── glidemark_app/                         # 视窗渲染层 (GPUI 原生界面)
        ├── src/
        │   ├── main.rs                        # 应用入口点与事件循环
        │   ├── lib.rs
        │   ├── actions.rs                     # 快捷键 Action 定义
        │   ├── state.rs                       # AppState 综合状态管理
        │   ├── editor_engine.rs               # 自研纯文本光标与行编辑引擎
        │   ├── theme.rs                       # 8 套调色板与色彩解析器
        │   └── views/
        │       ├── mod.rs
        │       ├── workspace.rs               # 主工作区、去框化工具栏与分屏布局
        │       ├── editor.rs                  # 源码编辑视图与行号渲染
        │       ├── preview.rs                 # Markdown 富文本渲染与自适应撑满排版
        │       ├── drawer.rs                  # 同级文件目录抽屉侧边栏
        │       └── settings.rs                # 偏好设置模态弹窗
        └── tests/                             # 视窗层集成测试
```

---

## 四、 常用命令与运维指南

### 1. 编译与本地开发
```bash
# 启动本地开发热运行
cargo run -p glidemark_app

# 运行整个工作区的全量自动化测试套件 (28 项用例全部通过)
cargo test --workspace
```

### 2. 重新编译图标与安装 App
```bash
# 1. 重新生成多分辨率渐变图标
python3 scripts/generate_icon.py
iconutil -c icns assets/icon.iconset -o assets/AppIcon.icns

# 2. 一键编译 release 优化版本并安装至系统 Applications
./scripts/bundle_macos.sh
```

---

## 五、 常用操作与全局快捷键一览

| 功能项 | 快捷键 / 鼠标操作 | 交互说明 |
| :--- | :--- | :--- |
| **纯源码模式** | `Cmd + 1` | 仅显示左侧源码编辑窗格与行号 |
| **左右分屏模式** | `Cmd + 2` | 左右并排显示，支持拖动中间分界线调节宽度比例 |
| **阅读预览模式** | `Cmd + 3` | 仅呈现排版渲染后的富文本正文，宽度响应式撑满 |
| **重置分屏** | 双击分界调节线 | 分屏宽度即刻精准恢复至 50/50 对称状态 |
| **侧边抽屉** | `Cmd + B` / 点击目录图标 | 展开/折叠当前文档同级目录下的所有 Markdown 文件列表 |
| **打开文件** | `Cmd + O` / 点击打开图标 | 调出原生非阻塞文件选择面板；或直接将文件拖拽进窗口 |
| **保存文件** | `Cmd + S` / 点击保存图标 | 保存当前编辑内容至物理磁盘；文件有修改时保存图标亮起强调色 |
| **重命名文档** | 双击顶栏文档标题 | 立即进入内联原地重命名，按 Enter 确认并完成系统重命名 |
| **导出独立 HTML** | `Cmd + Shift + E` / 点击导出图标 | 导出携带全量离线样式的独立单文件 `.html` |
| **设置面板** | `Cmd + ,` / 点击控制台图标 | 调出设置模态面板，即时切换主题外观、8套调色板与 10 国语言 |

---

## 六、 提交至 Gitee 新仓库操作指南

当前本地分支 `master` 已完整保留全部干净清晰的 Git 提交历史。将本项目推送至 Gitee 全新仓库的标准化步骤如下：

### 1. 在 Gitee 上新建空仓库
1. 登录 [Gitee (码云)](https://gitee.com/)。
2. 点击右上角 `+` -> **新建仓库**。
3. 仓库名称填入：`GlideMark`（介绍可填写：*High-performance, GPU-accelerated Markdown Reader and Editor in Rust + GPUI*）。
4. **注意**：选择**公开**或**私有**，**不要勾选**“使用 Readme 初始化”、“添加 .gitignore”或“添加开源许可证”，保持仓库为完全空白的空库。
5. 点击创建，获取仓库的远程地址（如 `git@gitee.com:<你的用户名>/GlideMark.git` 或 `https://gitee.com/<你的用户名>/GlideMark.git`）。

### 2. 在本地执行推送命令
在终端本工程根目录下执行以下命令即可一键完成全量推送：

```bash
# 1. 确保全部改动已提交至本地分支
git add -A
git commit -m "feat: complete visual redesign with MiaoYan theme, borderless toolbar, flex scroll fix and app bundle"

# 2. 添加 Gitee 远程源 (将 <YOUR_GITEE_URL> 替换为你的真实仓库地址)
git remote add origin <YOUR_GITEE_URL>

# 3. 推送 master 分支至远程
git branch -M master
git push -u origin master
```
如需后续添加其他镜像或继续推送，只需直接 `git push` 即可。
