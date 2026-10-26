# ADR 0014: 采用标准 Apple HIG 纯白圆角 Squircle 与多级缓存刷新机制彻底消除 Dock 灰色边

## 状态
已接受 (Accepted)

## 上下文 (Context)
1. **Dock 栏图标“灰色边 / 嵌套方框”根本原因**:
   - 在 ADR 0013 中曾尝试将 1024x1024 画布整张铺满不带透明角的纯白（Full-bleed pure white solid rectangle）。
   - **机制盲区**：macOS Dock **不会**对直角无 Alpha 透明通道的方形位图进行裁切。macOS 判定直角图片为未适配现代规范的遗留位图，因此强制在下方渲染默认的毛玻璃圆角托盘容器（System Dock Tile，半透明灰底），并将直角方形图标缩小约 20% 嵌入其中。
   - 这导致白色方块被放置在灰色圆角托盘中，四角与四周露出半透明灰色底边（如用户截屏 `media_1792983704860_f5e8c469.png` 所示）。
2. **Apple 官方现代应用标准图标规范 (macOS HIG)**:
   - 官方应用（如备忘录 Notes、App Store、设置 Settings、Claude 等）图标文件在 1024x1024 画布中：
     - 外围四周均为 100% 透明像素（Alpha = 0）；
     - 核心主体为 824x824 圆角超椭圆（Squircle，圆角半径 ~185px）；
     - 下方带有标准自然下投环境阴影（Y 轴偏移 +16px，高斯模糊 18px，Alpha 约 18%）；
     - 边缘配备 0.5pt-1pt 微发丝描边（`rgba(0, 0, 0, 0.08)`），确保在纯白壁纸下边缘清晰；
     - 此时 macOS Dock 识别其为标准 Squircle 图标，**直接原样渲染自身**，绝不套用任何系统灰色底座！

## 决策 (Decision)
1. **重构 `scripts/generate_icon.py` 严格遵循 Apple HIG 几何规范**:
   - **透明画布**：4096 x 4096（4x 超采样，最终降采样至 1024 x 1024），底色设为 `RGBA(0, 0, 0, 0)`。
   - **自然下投环境阴影**：在 `(100, 116, 924, 940)` 区域绘制圆角矩形并使用高斯模糊，模拟 macOS 标准光源。
   - **纯白超椭圆底板**：在 `(100, 100, 924, 924)`（824x824）绘制纯白卡片，圆角半径 185px，附带 1px 细微灰线描边（`rgba(0,0,0,0.08)`）。
   - **品牌梯度 Z 字形居中**：按标准 HIG 留白比例（500x500，占卡片约 60%）居中渲染暖色红褐至金黄琥珀渐变，呼吸感自然，契合 macOS 现代桌面美学。
2. **双重格式编译与全面缓存刷新**:
   - 同时生成 `AppIcon.icns`（供旧版/备用解析）与 `Assets.car`（通过 `actool` 编译现代资产目录，支持高分屏与系统级圆角识别）。
   - 在 `scripts/bundle_macos.sh` 中完善 LaunchServices 注册（`lsregister -f`），并精准清理 `$(getconf DARWIN_USER_CACHE_DIR)/com.apple.dock.iconcache` 与 `com.apple.iconservices*` 缓存，重启 Dock 立即生效。

## 结果 (Consequences)
- Dock 栏图标彻底消灭任何灰色边缘、灰色底座或多层套娃现象，在所有浅色、深色、纯色壁纸下均完美贴合 macOS 原生单层 Squircle。
- 自动化构建脚本支持一键生成全套 16x16 至 512x512@2x 资源并自动编入 App Bundle。
