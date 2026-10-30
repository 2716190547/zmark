# ADR 0031: 侧栏中心导航、无 Tab 头部与多根工作区

## 状态
提议 (Proposed) - 设计已确认，A/B 两期落地后改为 Accepted

## 上下文 (Context)
现有实现把“文档切换”放在右侧 Tab Strip 上，工作区只支持一个根目录，`open_file` 遇到根外文件还会自动替换根。实际使用中暴露出三个问题：

1. 多标签页占用右侧头部空间，文档标题、dirty 状态与窗口拖拽区相互挤压。
2. 单个工作区根无法同时浏览多个相关目录，用户只能反复“切换文件夹”。
3. Tab 是唯一的切换/关闭入口，去掉后文档生命周期没有替代方案；且现有 `close_tab` 不检查 dirty，存在丢改动风险。

用户明确要求：右侧只显示当前文档大标题（支持双击改名），文档切换与打开收敛到左侧文件树，左侧支持同时添加多个文件夹并排折叠展开。

## 决策 (Decision)

### 1. 移除右侧 Tab Strip，改为当前文档标题
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

### 3. 显式多根工作区 + 外部文件
- `workspace_roots: Vec<WorkspaceRoot>` 取代单个 `workspace_folder`；根由用户显式添加，可同时存在多个。
- 打开不属于任何根的文件时，不自动加根，放入“外部文件”虚拟组。
- 添加根前先 canonicalize；与已有根存在父子/重叠关系时拒绝并提示，避免同一文件重复展示。
- 两个同名根（如 `projA/docs` 与 `projB/docs`）在根节点右侧显示浅灰父目录后缀。
- 移除根不关闭其下文档；已打开文档（含 dirty）转入“外部文件”并标注“已移除工作区”，禁止随根丢弃。
- 根目录跨启动持久化；缺失目录保留并标记 `missing`，扫描时跳过且不 panic。

### 4. 扫描预算与刷新
- 初始只扫描根 + 第一层，展开目录时按需扫描下一层。
- 全局预算 2,000 entries / 400 Markdown files；单根 800 / 150；超限显示“已截断”。
- 切换文档不重扫所有根；文件操作或显式刷新只刷新对应根。
- 保留 symlink 不跟随、隐藏目录/`target`/`node_modules` 跳过。

### 5. 分期落地
- **A 期**：无 Tab、标题重命名、DocKey、脏关闭/退出保护、草稿组、侧栏 active/open/dirty 状态；内部使用单根。
- **B 期**：多根添加/移除、外部文件组、根持久化、缺失根、懒加载扫描、i18n 与 README 收尾。

## 结果 (Consequences)

- 右侧头部更纯粹，文档切换统一由侧栏承担；代价是侧栏成为主入口，必须清晰展示 active/open/dirty 状态。
- 多根让相关目录可以并排浏览；代价是根去重、嵌套拒绝、移除后文档归属、扫描预算都需要显式规则。
- 关闭/退出的 dirty 保护会打断“快速关闭”的操作，但避免隐藏文档丢改动。
- ADR 0021 的 Tab Strip 部分被本文取代；`open_docs` 资源模型保留。ADR 0027 的右侧头部“标签页”描述同步失效一部分。
- 新增术语：`Active Document`、`Workspace Root`、`Loose File`、`Scratch Document`；`Document Tab` 废弃。
