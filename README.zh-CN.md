# ZMark

<img src="assets/icon.iconset/icon_256x256.png" alt="ZMark 图标" width="128">

[English](README.md)

ZMark 是一款面向本地文件的 Markdown 编辑器与阅读器，使用 Rust 和 GPUI 构建。你可以编辑源码、阅读排版预览，或在分屏中同时使用两者。

## 功能

- 源码、分屏和预览三种模式，支持同步滚动。
- 抽屉支持多个工作区文件夹、外部文件、草稿、搜索和文档大纲。
- 支持 Markdown 表格、任务列表、带语法高亮的代码块和 Mermaid 图表。
- 支持浅色与深色外观、多套调色板和多语言设置。
- 可导出适合离线阅读的独立 HTML 文档。

## 开始使用

从 [发布仓库](https://github.com/2716190547/zmark-releases/releases) 下载可用版本，或按下方说明从源码构建。仓库包含 macOS、Linux 和 Windows 的桌面构建流程；可用版本和平台行为以实际发布的构建为准。

在 macOS 上，使用 `Cmd+O` 打开 Markdown 文件，使用 `Cmd+1`、`Cmd+2` 或 `Cmd+3` 切换视图，使用 `Cmd+,` 打开设置。`Cmd+S` 保存，`Cmd+Shift+E` 导出 HTML。设置窗口列出了常用快捷键。

## 从源码构建

安装当前稳定版 Rust 工具链及平台依赖，然后运行：

```sh
cargo run -p zmark_app
cargo test --workspace
```

[桌面构建工作流](.github/workflows/build-desktop.yml)列出了 Linux 构建依赖与打包步骤。在 macOS 上，`./scripts/bundle_macos.sh` 会构建并安装应用程序包。

## 技术栈

ZMark 使用 **Rust** 编写应用，使用 **GPUI** 构建界面，使用 **pulldown-cmark** 解析 Markdown、**syntect** 实现代码高亮，并用 **rusty-mermaid** 渲染图表。图片由 Rust 的 `image` 库解码。

更多细节见[架构决策](docs/adr/README.md)和[项目术语表](GLOSSARY.md)。
