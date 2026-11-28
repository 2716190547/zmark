# ADR 0031: 侧栏中心导航、无 Tab 头部与多根工作区

## 状态
已接受 (Accepted) - A/B 两期均已落地；B1–B5 全部通过审查验收，多根工作区、外部文件组、共享扫描预算与多语言侧栏已交付

## 上下文 (Context)
现有实现把“文档切换”放在右侧 Tab Strip 上，工作区只支持一个根目录，`open_file` 遇到根外文件还会自动替换根。实际使用中暴露出三个问题：

1. 多标签页占用右侧头部空间，文档标题、dirty 状态与窗口拖拽区相互挤压。
2. 单个工作区根无法同时浏览多个相关目录，用户只能反复“切换文件夹”。
3. Tab 是唯一的切换/关闭入口，去掉后文档生命周期没有替代方案；且现有 `close_tab` 不检查 dirty，存在丢改动风险。

用户明确要求：右侧只显示当前文档大标题（支持双击改名），文档切换与打开收敛到左侧文件树，左侧支持同时添加多个文件夹并排折叠展开。

## 决策 (Decision)

### 1. 移除右侧 Tab Strip，改为当前文档标题

> 2026-11-28 修订：默认仍无标签；ADR 0040 增加可在设置中开启的标签条。

- 右侧 38px 头部只显示当前文档名、dirty 点、视图模式切换与保存/导出/设置。
- 标题为纯文档大标题（文件名 / 未命名草稿），不显示面包屑路径；标题旁只有 dirty 点。
- 双击标题原地重命名；空白区保留 `start_window_move()` 拖拽，标题文字不参与拖拽。
- **不加大标题关闭图标**：关闭入口收敛为 `Cmd+W` 与侧栏树项 hover 的微型 ×；GPUI 0.2 无原生右键菜单，不设计右键菜单。
- `Cmd+W` 关闭当前文档，`Cmd+N` 新建草稿，移除 `Cmd+T` 的“新建标签页”语义。

### 2. 文档生命周期使用稳定 DocKey
- `DocKey` 区分 `Path(PathBuf)` 与 `Scratch(id)`；`OpenDoc { key, doc }`；`active: Option<DocKey>`。
- 不再用 `active_doc_idx` 作为唯一身份，避免关闭、根移除、loose 迁移时索引错位。
- 关闭 dirty 文档必须 Save / Discard / Cancel；关闭最后一个文档显示空状态，不自动生成 scratch。
- 退出应用时检查所有打开文档的 dirty 状态：GPUI 0.2 的 `on_window_should_close` 必须同步返回 `bool`，存在 dirty 时先返回 `false`，弹出 Save / Discard / Cancel 模态；确认后调用 `window.remove_window()`，不在回调内等待异步确认。
- 关闭后台 dirty 文档前，先将其切换为当前文档再弹出确认，避免用户在看不到内容时盲选。
- 草稿进入侧栏“草稿”虚拟组；无路径草稿保存时走原生 Save As，保存成功后 `DocKey::Scratch` 原地升格为 `DocKey::Path`，并按路径归入对应根或外部文件组。

### 3. 显式多根工作区 + 外部文件 (B 期深化)
- **精简数据模型（Ponytail 极简）**：
  ```rust
  pub struct WorkspaceRoot {
      pub path: PathBuf,           // canonicalized 绝对物理路径
      pub tree: Vec<FileTreeNode>, // 扫描得到的一级及展开子节点树
      pub is_expanded: bool,      // 是否折叠
      pub is_missing: bool,       // 磁盘路径不存在（断开/删除）
      pub truncated: bool,        // 是否触碰预算上限被截断
  }
  ```
- **派生状态替代冗余存储**：
  - `display_name`：由辅助函数 `root_label(&roots, idx)` 动态计算。同名根向上提取父目录名（如 `docs — repoA` 与 `docs — repoB`），无重名直接展示文件夹名。
  - `loose_docs`：侧栏“外部文件”列表由 `open_docs` 动态计算（过滤出路径不属于任何非 missing 根的文档），不单独设存储字段。
  - 侧栏树项的 `is_open` / `is_active` / `is_dirty` 均在渲染层根据 `open_docs` 动态派生。
- **根目录边界与安全**：
  - 添加根前执行 `canonicalize` 物理路径规范化。
  - 双向嵌套拒绝：若新路径为已有根的子目录，或新路径包含已有根，拒绝添加并报错。
  - 缺失根恢复：跨启动加载不存在的根标记 `is_missing = true`（置灰、不 panic、允许悬停 `×` 移除）；用户重新添加一个已存在但处于 `is_missing` 状态的根时，直接复活并重新扫描，不报 `AlreadyExists`。
  - missing 根下已打开的文档自动流转至“外部文件”虚拟组。
- **移除根绝不丢数据**：
  - 移除根只改变工作区根列表，**严禁强制关闭其下打开的文档**；
  - 所有已打开的文档（包含未保存的 dirty 文档）自动转入“外部文件”虚拟组继续保持编辑。
- **打开与保存行为纠偏**：
  - 彻底移除 `open_file` 与 `save_doc_as` 中“自动切换工作区根”的副作用；
  - 打开或另存为到根外文件时，作为外部文件保留，绝不篡改用户的 `workspace_roots`。

### 3.1 侧栏 UI 入口与视觉分级规范
- **添加文件夹入口上移**：
  - 彻底废除旧版目录行上的文本按钮 `[切换]`；
  - 移至侧栏顶部 38px 工具栏（位于 `收起侧栏`、`文件树`、`大纲` 图标右侧），显示为常驻的 `[+]` 文件夹图标按钮，点击直接唤起系统原生目录选择器，支持快捷键调用。
- **清晰排版字号分级**：
  - **一级根目录（Workspace Root）**：提升为 **13px Semibold**，深色文字（`theme.text_primary`），配备显式折叠/展开旋转箭头与文件夹图标，悬停右侧显示微型 `[×]` 移除按钮。
  - **二级虚拟分组（Drafts / Loose Files）**：提升为 **11px Medium**，柔和浅灰（`theme.text_muted`），上下留白与段间距适度加大，区隔明显。
  - **三级文档列表项（File & Doc Rows）**：正文字号保持 **12px Regular**，保持 16px 树级缩进与微胶囊高亮。
- **多根联合展开**：
  - 各根节点拥有独立展开/折叠状态；
  - 移除根时已打开文档无缝沉底至“外部文件”分组，绝不中断用户编辑。

### 4. 扫描预算与事件驱动刷新 (B 期深化)
- **彻底删除旧扫描管道**：
  - 物理删除 `sibling_files` 与 `scan_markdown_files`；
  - 删除 `refresh_sibling_files` 复合函数，彻底消除切换/打开文档时的双重扫描与 N 倍放大。
- **共享 ScanBudget 与公平性保底**：
  - 引入共享预算结构体 `ScanBudget`（默认上限 2000 entries / 400 markdown files；单根 800 / 150）；
  - 实施**首层公平保底**：每个根预分配首层保底额度（100 entries / 30 files），避免前序大根吃满全局预算导致后续根饥饿空白；
  - 预算用尽的根标记 `truncated = true`，未分配到预算的子项显示“已截断”；
  - 懒展开仅在用户点击折叠箭头时对该子目录续扫，继续消耗剩余预算，不重置预算池。
- **事件驱动局部刷新**：
  - `switch_document` 与 `close_document` **禁止触发任何文件树扫描**；
  - `open_file`、`rename_file`、`save_doc_as` 仅刷新目标文件所在的单一工作区根（`refresh_root(idx)`）；
  - 仅在添加/移除根、或用户主动触发时才执行全局扫描。

### 5. 分期与 Ticket 落地规划
- **A 期（已交付）**：无 Tab、纯大标题、DocKey 状态模型、脏关闭/退出保护、草稿组、侧栏 active/open/dirty 状态；单根内部过渡。
- **B 期（已全部交付）**：
  - **Ticket B1 ✅ (Root Model & Persistence)**：`WorkspaceRoot`、`Preferences.workspace_roots`、路径 canonicalize、嵌套/包含校验、missing 恢复、添加/移除根方法及全量独立单元测试。
  - **Ticket B2 ✅ (Scan Budget & Lazy Tree)**：`ScanBudget`、首层保底、懒展开续扫、truncated 状态、局部刷新机制及测试。
  - **Ticket B3 ✅ (State Integration)**：删除 `workspace_folder` / `sibling_files`、解耦 `open_file` / `save_doc_as` 换根副作用、外部文件动态派生、移除根保文档测试。
  - **Ticket B4 ✅ (Drawer UI & i18n)**：工作区 Header（[+] 添加）、多根折叠/展开/移除/missing 渲染、外部文件组渲染、新增多语言收敛至 `I18nStrings`。
  - **Ticket B5 ✅ (Integration & Polish)**：深层文件定位自动展开、重命名/保存失败错误提示、README 与 ADR 最终更新、打包 `/Applications/GlideMark.app` 终验。

## 结果 (Consequences)

- 右侧头部更纯粹，文档切换统一由侧栏承担；侧栏清晰展示 active/open/dirty 状态与多根层级。
- 多根让相关目录可以并排浏览；通过派生状态与 ScanBudget 保障了多根下的内存占用与秒级响应。
- 去除 `sibling_files` 彻底消除了无谓的重复扫描，编辑与切换文档零磁盘 I/O 抖动。
- 移除根保文档与退出保护形成严密的数据安全护栏。
