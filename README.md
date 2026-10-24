# GlideMark

> **GlideMark** 是一款由 **Rust + GPUI** 强力驱动的单文档 GPU 原生极速 Markdown 阅读与编辑器。以现代、纯粹、克制的极简设计语言（参考妙言 MiaoYan），提供行云流水般的书写与排版阅读体验。

---

## 特性亮点

- **GPU 硬件加速渲染**：基于 Zed 同款的 GPUI 渲染引擎构建，极致轻量，告别 Electron 套壳的臃肿内存与卡顿。
- **妙言 (MiaoYan) 极简美学**：纯白呼吸感画布（`#FFFFFF`）配合细致入微的浅灰发丝分割线（`#EDEDED`）与清爽的鲜绿强调色（`#2DA44E`）。
- **去框化极简工具栏**：顶栏功能图标无硬框常态融入界面，悬停呈现圆润微胶囊，聚焦呈现强调色，零 Emoji 严肃视觉规范。
- **三重视图自由切换**：
  - **源码模式 (`Cmd+1`)**：专注编写，自研纯文本与行光标引擎。
  - **左右分屏 (`Cmd+2`)**：边写边看，5px 舒适可拖拽分界条（双击快速回正至 50/50）。
  - **阅读预览 (`Cmd+3`)**：正文自适应撑满，极佳阅读体验。
- **系统级原生拖拽与双击重命名**：
  - 支持直接将 `.md` 文件拖拽进窗口瞬间打开。
  - 双击顶部标题直接内联原地重命名文件。
- **多主题与多语言**：
  - 8 套精美调色板：MiaoYan, Claude Warm, macOS Vibrant, Tokyo Night, Nord, GitHub Light, Solarized Light, One Dark Pro。
  - 支持浅色/深色/跟随系统外观。
  - 10 国主流语言完整支持。
- **离线独立 HTML 导出**：一键生成自包含样式的单文件 HTML，脱机即阅。
- **macOS 原生独立应用**：已预编译封装为标准 App Bundle（`/Applications/GlideMark.app`），搭配精美红褐-金黄渐变折叠 Z 原生图标。

---

## 快速上手

### 运行环境
- macOS 12.0+ (Apple Silicon / Intel)
- Rust 1.76+

### 本地编译与运行
```bash
# 启动本地开发
cargo run -p glidemark_app

# 运行自动化测试套件
cargo test --workspace
```

### 打包安装至系统应用程序
```bash
./scripts/bundle_macos.sh
```
安装完成后，可在 **启动台 (Launchpad)** 或 `/Applications` 中直接点击打开 GlideMark，或右键关联 Markdown 文档。

---

## 快捷键速查

| 操作 | 快捷键 |
| :--- | :--- |
| **源码模式** | `Cmd + 1` |
| **分屏模式** | `Cmd + 2` |
| **阅读模式** | `Cmd + 3` |
| **展开/折叠目录抽屉** | `Cmd + B` |
| **打开文件** | `Cmd + O`（或拖拽文件进窗口） |
| **保存文件** | `Cmd + S` |
| **重命名文档** | 双击顶栏标题 |
| **重置分屏比例** | 双击分界线 |
| **导出独立 HTML** | `Cmd + Shift + E` |
| **偏好设置** | `Cmd + ,` |

---

## 详细交接与技术架构

详见工程交接文档：[`HANDOVER.md`](./HANDOVER.md) 与架构决策记录 [`docs/adr/`](./docs/adr/)。
