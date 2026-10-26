# ADR 0015: 引入 Apple Icon Composer (`.icon`) 架构编译 `IconImageStack` 彻底脱离 macOS Tahoe "Squircle Jail"

## 状态
已接受 (Accepted) - 替代 (Supersedes) ADR 0014

## 上下文 (Context)
1. **需求对齐核实**:
   - 用户的视觉需求自始至终高度一致明确：“Dock 栏图标只要单一层级、纯白底板、不要任何灰色托盘或外层套娃边框，必须与 macOS 系统原生应用（如备忘录 Notes、妙言 MiaoYan）视觉标准统一”。
2. **反复出现灰色边框（“一层套一层”）的真正技术根因**:
   - 当前操作系统为 **macOS 27.2 (macOS Tahoe)**。
   - 在 macOS Tahoe 以前的版本中，系统允许直接读取传统的扁平 `.appiconset` PNG 或 `.icns`。
   - 但在 macOS Tahoe 中，苹果推行了严格的 Dock Squircle 网格统一规范：
     - 若应用仅提供传统扁平位图（即缺少现代分层资产定义），系统将强制判定其为“未完全适配的遗留应用”；
     - 系统会将该图标判定落入 **"Squircle Jail"（灰色方形托盘牢笼）**，自动在其底层垫上一个系统级半透明灰色圆角玻璃托盘，并将图标本身等比缩小约 20% 嵌入其中。
     - 这导致无论在位图层面如何微调画布尺寸（824x824、1024x1024 或带阴影圆角），系统只要检测不到现代分层资产，就会无条件套上灰色托盘。
3. **对照逆向分析标杆应用（妙言 MiaoYan）的实现机制**:
   - 逆向解构 `/Applications/MiaoYan.app/Contents/Resources/Assets.car` 发现，妙言并未使用扁平 PNG 打包，而是使用了 Apple 现代 **`IconImageStack`** 结构：
     - **Layer 1 (底板 Fill)**：直接声明背景色（`Color components: [0.995, 0.995, 0.995, 1]` 纯白）；
     - **Layer 2 (前景 Group)**：透明背景的品牌矢量符号；
   - 当 macOS Tahoe 渲染此类图标时，系统将直接提取 Layer 1 的纯白色作为 Dock Squircle 的背景色，并结合 Liquid Glass 材质进行单层无缝渲染，**绝不会触发灰色玻璃托盘**。

## 决策 (Decision)
1. **全面弃用传统扁平 `.appiconset` 编译路径**:
   - 移除此前在 Python 中手动绘制外层阴影与假 Squircle 边框的 workaround 代码。
2. **采用 Apple 现代 Icon Composer 包格式 (`assets/AppIcon.icon`)**:
   - **`Assets/glyph.png`**：仅负责高质量品牌前景图层（暖色红褐至金黄琥珀渐变 Z 字形，4x 超采样 4096 降采样至 1024）；
   - **`icon.json`**：严格遵循 Icon Composer 规约：
     - `"fill"`: 声明 `"automatic-gradient": "extended-srgb:1.00000,1.00000,1.00000,1.00000"`（纯白底座）；
     - `"groups"`: 包含单层前景 `"image-name": "glyph.png"`；
     - `"supported-platforms"`: 声明 `"squares": ["macOS"]`。
3. **通过 Xcode 27 官方 `actool` 编译现代 `IconImageStack`**:
   - 调用 `actool assets/AppIcon.icon --compile assets --platform macosx --minimum-deployment-target 13.0 --app-icon AppIcon`；
   - 自动在 `assets/Assets.car` 中生成包含 `NSAppearanceNameAqua`、`NSAppearanceNameDarkAqua` 和 `ISAppearanceTintable` 的原生 `IconImageStack` 资产，以及兼容版 `AppIcon.icns`。
4. **Bundle 安装与系统级缓存清理**:
   - 将现代 `Assets.car` 与 `AppIcon.icns` 部署入 `/Applications/GlideMark.app/Contents/Resources/`；
   - 清理 LaunchServices 注册与 `com.apple.dock.iconcache`、`com.apple.iconservices*` 缓存。

## 结果 (Consequences)
- 从系统底层架构上满足 macOS Tahoe 的现代 Squircle 标准，彻底根治“灰色外框 / Squircle Jail”。
- 避免了在位图上绘制虚假阴影或描边可能带来的多重套娃副作用。
