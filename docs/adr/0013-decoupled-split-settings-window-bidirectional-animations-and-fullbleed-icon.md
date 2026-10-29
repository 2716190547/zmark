# ADR 0013: 解耦独立分栏设置窗口与双向平滑抽屉动画

## 状态
已接受 (Accepted) - 吸收合并 0010（分段控件与自动保存）、0011（跨窗口同步）、0012

## 上下文 (Context)
1. **用户明确偏好独立分栏设置窗口**:
   - 用户要求保留独立弹窗与左右分栏的设置体验（类似系统偏好设置与原生桌面工具），拒绝嵌入式侧抽屉方案。
2. **侧边栏关闭时缺失平移动画**:
   - 此前展开侧边栏时使用了动画，但收起时直接使用条件卸载，导致抽屉在 0ms 瞬间闪断消失。
3. **架构沉淀说明**:
   - 图标规范最终方案见 [ADR 0015](0015-icon-composer-iconimagestack-bypass-squircle-jail.md)；默认主题配色方案见 [ADR 0021](0021-multi-tab-and-workspace-directory-architecture.md)。

## 决策 (Decision)
1. **解耦异步独立分栏设置窗口 (`SettingsWindowView`) 与配置自动保存**:
   - **彻底杜绝闪退**：通过 `cx.defer` 异步派发窗口创建指令，避开事件监听堆栈上的 `&mut WorkspaceView` 借用；`SettingsWindowView` 拥有独立的状态拷贝与 `WeakEntity<WorkspaceView>`，在自身的 `render()` 中绝不读取主工作区，彻底消除 Rust 借用竞争。
   - **两栏排版布局**：
     - 左侧导航栏（160px，`theme.bg`）：外观、编辑器与保存、语言、关于四大模块胶囊导航；
     - 右侧内容区（flex-1，`theme.surface`）：独立滚动容器，包含原生无框分段控件（源码/分屏/预览）、8 套调色板选择器、1000ms 防抖自动保存（输入停顿 1s 自动落盘与失焦即时保存）、10 国语言切换及快捷键概览；
     - 顶部预留 80px macOS 红黄绿交通灯安全区，拥有专属窗口生命周期与快捷键唤起。
   - **跨窗口双向同步**：设置窗口中的配置修改通过模型句柄直接写回磁盘 TOML，并通过 GPUI `notify` 即时同步主工作区渲染，无需重启。
2. **双向 180ms ease-in-out 平滑动效 (Bidirectional Drawer Animation)**:
   - 在工作区维护 `drawer_closing` 状态。
   - 展开时：`180ms` 从 `0px` 平滑过渡至 `250px`；
   - 收起时：先激活 `drawer_closing`，执行 `drawer_slide_out` 动效（从 `250px` 平滑回缩至 `0px`），并在 180ms 定时器完成后优雅卸载。

## 结果 (Consequences)
- 原生独立分栏设置窗口运行稳定，支持 `⌘,` 随时呼出与激活，输入内容自动持久化防丢失。
- 侧边栏展开与收起均具备流畅平滑的 60fps 动效，告别生硬突兀感。
