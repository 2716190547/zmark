# ADR 0030: macOS 研发工具链分工、Instruments 性能诊断与零成本 Ad-Hoc 签名

## 状态
已接受 (Accepted)

## 上下文 (Context)
在评估是否需要完整安装 Xcode 套件以及如何规划 macOS 原生开发工作流时，梳理出以下核心权衡：
1. **Xcode IDE 与 Rust 开发模式脱节**：
   - Xcode.app 占用 15GB~30GB 磁盘空间，且对 Rust 语法与 Cargo 生态支持羸弱；日常代码编写与编译通过终端命令行（`cargo` + 现代编辑器）更高效迅捷，无需引入 `cargo-xcode` 生成笨重的 `.xcodeproj`。
2. **底层 SDK 与诊断工具不可替代**：
   - 官方多层图标编译工具 `actool`（构建 ADR 0015 的 `Assets.car`）高度依赖 Xcode 开发者目录（`/Applications/Xcode.app/Contents/Developer`）；
   - GlideMark 基于 GPUI 与 Metal 硬件加速构建，在追求 ProMotion 120Hz 极致丝滑、复杂流程图渲染与超大文档排版时，排查掉帧与 CPU/内存瓶颈必须依赖苹果官方的 **Instruments（Time Profiler、Metal System Trace、Allocations）**，这是开源分析器在 macOS 架构下无法比拟的黄金探针。
3. **分发成本与签名顾虑**：
   - 开发者未订阅苹果开发者计划（每年 $99 / 688 元），主要用于本机日常使用与开源共享；
   - 未签名的应用在 macOS 下容易被 Gatekeeper 与 LaunchServices 拦截或误报“已损坏”，需要一种**零费用、免账号**但又能获得系统级完整信任的签名方案。

## 决策 (Decision)

### 1. 工具链“轻重解耦”分工原则 (Headless Toolchain Paradigm)
- **日常编写与编译**：完全通过终端 `cargo run` / `cargo build` 进行，不强制使用 Xcode GUI IDE 打开工程；
- **底层 SDK 与工具支撑**：保留安装 Xcode 套件，为系统提供官方完整的 macOS SDK、`actool` 资产编译器与 LLDB 调试器。

### 2. 零成本 Ad-Hoc 本地代码签名 (Zero-Fee Ad-Hoc Code Signing)
- 在 [`scripts/bundle_macos.sh`](../../scripts/bundle_macos.sh) 打包安装流程中注入苹果官方 Ad-Hoc 签名命令：
  ```bash
  codesign --force --deep --sign - "/Applications/GlideMark.app"
  ```
- **核心价值**：
  - **100% 免费**：无需购买 Apple Developer 账号，本地即刻完成；
  - **密封资源封包**：将可执行 Mach-O 二进制、`Info.plist` 与 `Resources`（`Assets.car`、`AppIcon.icns`）形成不可篡改的校验哈希（Sealed Resources）；
  - **消除系统误报**：彻底解决 macOS 偶发的“应用已损坏，请移至废纸篓”错误，确保 Dock 栏、Spotlight 聚焦搜索与启动台能稳定持久化关联应用与 Markdown 打开权限。

### 3. 一键 Instruments 性能诊断脚本 (`scripts/profile_fps.sh`)
- 新增轻量级性能录制脚本 [`scripts/profile_fps.sh`](../../scripts/profile_fps.sh)：
  - 通过 `xcrun xctrace record --template 'Time Profiler'` 启动 Instruments 诊断引擎；
  - 录制指定秒数（默认 8s）内在 GlideMark 窗口内快速滑动、大文档加载或主题切换的真实调用栈；
  - 录制结束后自动在 Instruments 图形界面中打开 `.trace` 分析看板，精准定位毫秒级掉帧元凶（如正则回溯、多余 AST 重建或 Metal 提交延迟）。

## 结果 (Consequences)
- 明确了 Xcode 在工程中“作为底层依赖与高级探针，而非日常编辑器”的轻量化定位。
- 实现了打包安装阶段 100% 免账号的合法本地签名，杜绝 macOS 拦截弹窗。
- 固化了 120fps 性能追踪工具链，使性能排查从“主观猜测”转变为“数据驱动”。
