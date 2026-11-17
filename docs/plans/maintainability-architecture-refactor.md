# 可维护性架构重构：渐进模块化、热路径去重与资源所有权

> 维护更新（2026-11-17）：文中 `scripts/bench_memory.sh` 为历史入口，已因读取正式配置且测量范围不足而删除。受控测量使用保留的 `phase_j_memory_probe`、`editor_bench`、`media_probe` 与帧探针；历史数字保留原口径。

> 当前代理约定（2026-11-16）：herdr-triad 已弃用；由当前代理自行审查，不向其他 pane 派任务。本文跨窗格与独立代理审查要求仅作历史记录，现行规则见 [代理工作约定](../agent-workflow.md)。

状态：**v1 规划 + 实施记录；R1/R2 已集成交付，R3/R4/R5 按证据暂缓**。  
基线：`ba71833dfc5bdac94575e16652fa8312df08d904`；工作区另有未提交的滚动修改，动工前必须先独立落地或在隔离 worktree 实施，不能把重构叠在脏 diff 上。

## 1. 目标与判断标准

按优先级实现：

1. **更好维护**：所有权和依赖方向清楚，修改滚动/图片/语法时不再都要进入 `WorkspaceView`。
2. **精简架构**：删除镜像状态、重复编译、重复解析/复制和无效失效；模块数量随真实职责增长，不为“分层”制造接口。
3. **提升性能**：优先减少 UI 线程的整篇文本复制、重复 shape/flatten、图片二次解码和同步 I/O。
4. **减少占用**：分别测量稳定 RSS、Physical Footprint、峰值、二进制大小和构建缓存；不拿 `target/` 大小或二进制大小冒充运行内存。

每个性能改动只有在同机、同文档、同窗口、同操作序列下出现明确收益才保留。目标改善幅度不是承诺值：

- 主目标改善 15%、其他主指标回退不超过 5% 是探索目标，结合重复测量噪声评估，并非用户确认的硬门槛。
- 若只是简化代码而性能基本不变，必须明确写“维护性收益，无实测性能提升”。
- 不通过增大缓存、关闭图片/高亮、降低输入规模或删除有效保护来制造收益。

## 2. 当前事实与主要问题

规划阶段只读检查，未运行新的性能基准，未修改生产代码。

### 结构事实

- 非测试 Rust 源码约 **21,542 行**。最大文件：`views/workspace.rs` **3,870 行**、`state.rs` **2,372 行**、`views/preview.rs` **2,228 行**、`views/drawer.rs` **1,927 行**。
- `WorkspaceView` 同时持有文档、焦点、编辑布局、预览选区、滚动、语法 warm/队列/cache、图片、灯箱、设置和弹窗状态；视口、资源和生命周期彼此耦合。
- `views/preview.rs` 反向导入 `views/workspace.rs` 的语法失败类型，说明共享类型的位置不正确。
- `main.rs` 再次 `mod actions/assets/editor_engine/...`，而 `lib.rs` 已导出同一批模块：代码在 lib/bin 两个 crate 中重复编译，12 个库内测试也重复运行。
- 当前有 **51 个集成测试文件、316 个 `test`/`gpui::test` 属性**。测试数量本身不是目标，重复链接、低价值源码字符串断言和重复行为覆盖才是成本。

### 可定位的重复工作

- `AppState` 同时持有 `open_docs` 与当前 `doc` 镜像。`sync_from_editor()` 每次把编辑器全文生成新 `String`，再把 `self.doc.clone()` 写回 `open_docs`；切换/打开文档也重复 clone。
- `DocumentLayoutCache::update()` 为未变行保存并克隆完整 `String`，然后重新生成整个 `Vec<VisualLine>`；缓存命中也伴随大量文本所有权复制。
- 预览列表项每次渲染执行 `flatten_block()`，并分别为鼠标 down/move 克隆块文本；`ParsedDocument.headings()` 也在渲染路径重新遍历 AST。
- 语法 cache 是 `Vec<CacheEntry>`；`get()` 为更新 LRU 做线性查找和移动，插入时重复累计堆字节。是否为性能瓶颈必须测量，不能仅凭复杂度下结论。
- 本地 raster 图片先由 `image` 完整解码，随后写临时文件，再由 GPUI `ImgResourceLoader` 解码一次；这是明确的二次解码和临时磁盘 I/O。GPUI 0.2.2 已有 `ImageSource::Render(Arc<RenderImage>)`，可逐步改成直接位图所有权。
- `AppState::new()` 同步 `refresh_all_roots()`；大工作区启动成本需要实测，不能预设一定要线程化。

### 资源口径

- 当前 release 可执行文件约 **19 MiB**；`target/` 约 **38 GiB**。后者是可再生构建缓存，不属于运行内存，也不在本方案中直接删除。
- 既有语法 cache 4 MiB、可见图片解码 cache 64 MiB、撤销历史 8 MiB 等硬预算保持不变。
- Phase J 的历史内存改善只能作为旧场景参考，不能当成本次基线或收益。

## 3. 目标架构（仍只有两个 crate）

不新增 `ui`/`infra`/`services` crate，不引入插件系统、事件总线、依赖注入、actor 框架或统一异步任务框架。

```text
glidemark_core（保持纯 Rust、无 GPUI）
  Markdown / AST / anchor / config / document / export / syntax primitives

glidemark_app
  main.rs                  只复用 glidemark_app lib，不重复 mod
  state/
    mod.rs                 AppState 组合、偏好和窄接口
    documents.rs           DocumentStore、active key、打开/切换/保存/关闭
    tree.rs                WorkspaceRoot、ScanBudget、目录扫描
    search.rs              文件名搜索任务、代际和展开状态恢复
  syntax.rs                WarmState、失败类型、请求快照、cache 所有权（无 UI 渲染）
  local_images.rs          图片读取/解码/预算/生命周期
  editor_engine.rs         活动文档编辑、撤销和行模型
  views/
    workspace/
      mod.rs               WorkspaceView 组合、顶层 render 和事件接线
      lifecycle.rs         parse/autosave/document/window 事件
      syntax.rs            GPUI task 调度、取消、准入和回调
      scrolling.rs         双向同步、滚动条拖动和失效
    preview/
      mod.rs               虚拟化块渲染
      selection.rs         文字区域、命中、选区和复制映射
      code.rs              代码块渲染
    editor/
      mod.rs               编辑 pane、实际布局和 cache
      tokenizer.rs         纯行级 Markdown tokenization
    drawer/
      mod.rs               文件树与 Drawer 渲染
      search_input.rs      现有 IME 输入组件（直接搬出，不重写）
```

依赖规则：

- `core` 不依赖 `app`；`state`/`syntax`/`local_images` 不依赖 `views`。
- `views/workspace` 可以协调 pane 和 app service；`preview`、`editor`、`drawer` 不得再反向导入 `workspace`。
- `WorkspaceView` 字段默认私有；测试需要的观测优先使用现有行为入口和 `test-observation`，不为了测试把所有字段重新公开。
- 不建立第二套 focus、revision、AST、图片或选区真源。

## 4. 分阶段实施

### R0：基线与保护（1 个小提交）

- 先独立处理当前未提交的滚动修改；重构从干净、可编译候选开始。
- 固定三类 release 场景：默认文档、1.5MB 长文档、含真实图片的长文档。复用 `scripts/bench_memory.sh` 与 Phase J 的隔离测量入口，不新建性能平台。
- 记录稳定/峰值 RSS 和 Physical Footprint；在能可靠观测时记录输入到绘制、滚动帧 p50/p95、解析/布局耗时。区分冷启动和热运行。
- 明确当前滚动问题的复现条件，不能把未复现的卡顿写成已修复。

**通过条件**：得到可重复的基线表；没有基线的性能项不进入“已优化”结论。

### R1：先做零行为变化的模块整理（3–5 个小提交）

1. `main.rs` 改为使用 `glidemark_app` lib，删除重复 `mod` 声明；验证启动、资产和快捷键完全一致。
2. 原样搬出 `drawer/search_input.rs`、`preview/selection.rs`、`editor/tokenizer.rs`，暂不改变算法。
3. 把 `WorkspaceRoot`/扫描和 `SearchJob` 从 `state.rs` 拆出；AppState 只保留组合和文档/偏好职责。
4. 将语法共享类型移到 app `syntax.rs`，切断 `preview -> workspace`；再把 GPUI 语法任务、滚动和生命周期从 `WorkspaceView` 分模块承载。

每个提交只做一种移动；允许短期 `pub use` 兼容旧路径，但最终删除重复包装，不永久保留两套入口。

**通过条件**：公开行为不变；依赖方向可检查；相关测试通过；lib/bin 不再重复执行同一组单元测试。

### R2：消除文档与布局的镜像/复制（2–4 个小提交）

1. 用 `DocumentStore { open_docs, active }` 替代 `AppState.doc` 镜像。活动文档以编辑事务中的 `EditorEngine` 为输入真源，同步完成后的 `OpenDoc` 供其他操作读取；非活动文档以 `OpenDoc` 为真源。通过受控入口访问，切换文档不再 clone 整份 `Document`。
2. `EditorEngine` 仍负责活动编辑和撤销，不引入 Rope。同步时把文本直接写入活动文档并尽量复用已有 `String` 容量，删除 `od.doc = self.doc.clone()`。
3. 布局 cache 的未变行不再每次克隆完整 `String`：优先复用 `Arc<str>` 行快照或稳定的行内容身份；必须验证修改、插入/删除、undo/redo 和前后缀复用，不能用未校验的 hash 相等代替文本相等。
4. 预览每个可见项只 flatten 一次，块文本在渲染和 down/move 回调间用 `Arc` 共享；大纲数据按 AST 代际复用，不每帧重新遍历。

**通过条件**：文档 dirty/conflict/撤销/自动保存完全保留；大文档编辑和切文档不再有可测的整份 clone；内存或编辑耗时有实测改善，否则只保留确实更简单的部分。

### R3：按测量优化热路径（2–4 个小提交，独立可停）

- **解析**：先记录 `parse_markdown` 在 UI 线程的耗时。只有大文档超过帧预算时，才把该档位移到后台，并沿用 `doc key + revision + request id` 防陈旧结果；小文档保持当前简单同步路径。
- **布局**：优化的是重复 shape/flatten，不重写文字排版；真实字体度量、CJK、选区和光标仍共用同一几何真源。
- **语法**：先测锁等待和 cache 成本。若渲染侧 LRU 更新确为热点，增加只读 peek/命中路径；只有 profiling 证明 Vec 是瓶颈才改数据结构，保留 4 MiB 和 retained 语义。
- **工作区扫描**：若启动扫描明显阻塞首帧，复用现有搜索的 generation/后台执行模式，不建立第二套任务系统。

**通过条件**：目标场景 p95 或稳定内存改善结合重复测量结果评价；没有改善的复杂化改动撤回。

### R4：图片直接位图所有权与释放（2–3 个小提交）

1. PNG/JPEG/GIF/WebP 静态首帧在现有受限解码后直接生成 `Arc<RenderImage>`，用 `img(render)` 显示，删除对应临时文件和 GPUI 第二次 raster 解码；SVG 先保留现有路径，避免一次改两种颜色管线。
2. `LocalImage` 直接拥有实际位图字节，64 MiB 预算按真实 retained 图像计算；文档切换、隐藏、刷新、Workspace 释放和 lightbox 关闭走同一条释放路径。
3. 验证 RGBA→BGRA、alpha、动图首帧和 SVG 既有颜色，不用“能显示”替代颜色验证。

**通过条件**：相同图片操作序列下解码次数/临时文件确实减少，稳定与峰值内存不回退；隐藏后资源可回收；I0/本地图片颜色与选区复制回归通过。

### R5：测试与交付成本收尾（1–2 个小提交）

- 纯算法测试就近放入模块 `#[cfg(test)]`，减少无必要的集成测试二进制和重复链接；真实 GPUI 几何、IME、选区、颜色和陈旧结果测试保持独立。
- 只删除有证据的恒真、重复或自造结果测试；不合并需要隔离全局状态的测试，不用 `ignore` 降门槛。
- 可测量一次 `thin LTO + strip` 的 release 体积/性能/构建时间，再决定是否采用；不为了体积删除 GPUI/图片/Mermaid 功能或盲目升级依赖。

**通过条件**：报告真实构建/链接/执行耗时和 passed/failed/ignored；测试减少不冒称覆盖提升。

## 5. 明确不做

- 不重写 GPUI 渲染、不换成统一 `uniform_list`、不调用 `measure_all()` 换滚动精度。
- 不引入 Rope、ECS、插件框架、事件总线、全局 service locator 或第三/第四个 crate。
- 不为每个函数建立 trait，不为只有一个实现的 service 建工厂。
- 不把所有缓存合成一个“大 CacheManager”；语法、图片、布局的失效和预算不同。
- 不把“文件变小/行数变少”当架构目标；允许为清晰所有权保留必要代码。
- 不在本方案顺带实现完整 Windows/Linux 适配，沿用现有 `platform.rs` 地基即可。

## 6. 风险与控制

- **文档真源改动**：dirty、冲突、后台 dirty 文档切换、撤销和自动保存最容易回归，必须先于性能改动保留行为检查。
- **Arc 生命周期**：`Arc<str>`/位图若被旧 AST/cache 持有，可能反而升内存；每处共享必须写明代际和释放时机。
- **后台解析**：旧 doc/revision 结果不得覆盖新输入；显式刷新、保存和模式切换语义保持一致。
- **直接位图**：绕过 GPUI resource loader 后必须自行保证 BGRA/alpha 和 atlas 释放，不能复用旧“已解码”结论。
- **模块搬迁**：测试大量直接读公开字段，先用行为入口替代，不通过扩大 `pub` 维持旧测试。
- **未提交工作**：不得覆盖当前滚动修改；每个阶段可独立停止和回退。

## 7. 推荐执行顺序

最高收益且风险最低的顺序是：

**R0 基线 → R1 去重复编译与模块边界 → R2 文档/布局去重 → R4 raster 图片二次解码 → R3 按测量专项优化 → R5 测试与发布收尾。**

R1/R2 完成后就应获得主要维护性收益；R3 不必为了“完成架构”全部实施。每个阶段单独提交、单独验证，发现没有收益即停止，不做大爆炸式重写。

## 8. 最终验收

- 结构：依赖方向无回环，`main.rs` 不重复编译模块，`AppState.doc` 镜像和明确的热路径重复复制被删除。
- 行为：中文 IME、编辑/撤销/自动保存、预览选区/复制、双向滚动、语法准入、图片/灯箱、设置与文件树保持。
- 性能：同场景基线对照，分别报告稳定/峰值 RSS、Physical Footprint 和可得的帧/解析指标；未测项明确写未测。
- 回归：阶段候选运行相关回归，最终候选按项目要求运行全量及观测专项，分报精确统计和重叠，不用“全绿”代替视觉或资源证据。
- 交付：最终 release 打包、签名和二进制时间核对；按用户已授权的范围更新现有 PR，保持打开，不自动合并。

## 9. 执行补充

- 性能基准不得直接用正式用户配置执行交互回放：沿用 Phase J 隔离入口，将配置、工作区、文档和图片复制到私有临时目录，记录原件前后 hash 和 mtime；现有 `bench_memory.sh` 的正式配置启动结果只能作为只读参考。记录采样次数和波动，15% / 5% 阈值需结合重复测量噪声判断。
- 现有未提交滚动修复保持原样，重构在独立 worktree 开展；集成前对照滚动候选逐项验证并解决冲突，不覆盖或重置现有改动。重构基线与最终集成代码不同，性能数字分别标明版本。
- 文档真源迁移必须覆盖未保存草稿、后台自动保存、另存为/重命名及切换中的在途任务，保留 `DocKey + revision` 校验与撤销、保存失败时的内容；按阶段提交可回退，不以简单搬移通过安全验收。
- 此前用户已要求最终 PR 和打包替换现有 app；方案第 8 节“不默认 push/PR”不覆盖该明确授权。交付前对最终集成候选完成回归与资源校验，更新现有 PR，保持打开，不自动合并。
- R0 不直接运行当前 `bench_memory.sh` 做交互回放；先确保隔离入口覆盖实际读取与写入路径，包括设置保存、工作区扫描及自动保存。
- R2 的“唯一真源”须细化：活动文档的编辑事务以 `EditorEngine` 为输入真源；每次接受编辑后，在允许解析、保存、切换及后台任务读取前同步到对应 `OpenDoc`，同一事务更新 revision、dirty 和 AST stale 状态；`last_saved` 只在保存成功后更新。非活动文档以各自 `OpenDoc` 快照为真源。不得给外部同时提供可绕过此事务的两套可写入口；如果现有调用无法保证先同步，须先明确和测试边界再迁移。
- 文档迁移回归覆盖未保存草稿、修改后切换及切回、重命名、另存为、关闭最后文档、保存失败、磁盘外部冲突、undo/redo、自动保存计时与切换/重命名/关闭交错、旧后台解析/保存回调的 key/revision 校验；不得为了消除 clone 缩短文档生命周期或跳过保护。
- R4 按现有调用事实区分资源：当前 raster 走预览，lightbox 承载 Mermaid SVG；不为不存在的 raster 灯箱共享建立资源管理器。直接位图方案须明确预览渲染和 GPUI atlas 对 `Arc<RenderImage>` 的持有、`App::drop_image` 的窗口上下文及最终可安全释放的时机；64 MiB 核算必须覆盖 loading、retired、旧代次及共享实例去重，且保留现有格式失败、超限、alpha/方向处理的实际语义。以真实颜色、并发预算与释放回归和实测为前提，不能仅凭 API 存在断言可行。
- 15%/5% 为计划中的探索目标，不是用户确认的硬门槛；用重复测量和波动评价结果，无法支撑可靠结论时如实报告，不为追指标引入复杂方案。

独立规划审查：修订版（审阅时 SHA-256 `56de04d99b21fc6c8bde72e9432a47682ba38ce00e8ec92de84345bf8550b666`）无未决实质异议；上述正文同步了审查建议，未改变已审核决策。

## 10. R0/R1 中间记录

在 `96bf2d4` 的隔离 release harness 中，使用 `AppState::new_isolated`、合成文档、1200×800 窗口和六秒空闲采样，各三次。基线均值 RSS：默认 90.91 MiB、1.5 MB 文本 170.08 MiB、含图片文档 196.31 MiB；Physical Footprint 分别约 43、103、127 MB。相同条件在 R1 模块拆分候选复测 RSS 分别 90.89、171.91、198.44 MiB，Footprint 分别约 43、103–104、126–128 MB。此采样不证明内存改善，不能排除时序噪声；图片是否全部解码、滚动后峰值及帧时间尚未测量。完整隔离数据保存在 `/tmp/glidemark-r0-baseline` 和 `/tmp/glidemark-r0-candidate`。

R1 阶段 `96bf2d4`、`eeeacbc`、`9409eef`：库入口复用、抽屉输入和编辑器 tokenization 模块拆分、共享语法类型、预览回调文本共享；独立审查未发现实质问题。候选 `cargo test --workspace` 为 317 passed / 0 failed / 0 ignored；Phase H 与 Phase I 观察专项合计 49 passed / 0 failed（包含与默认全量重叠的测试）。这些提交在隔离 worktree，尚未与原工作区滚动改动集成或交付。

R4 预研：GPUI 0.2.2 macOS `metal_atlas.rs` 的 `remove` 在共享 texture 仍有其他 tile 时递减引用计数但保留该 key；重复释放与重用的实际影响需要在真实 Metal atlas 验证。现有图片方案也使用该框架路径，不能以源码推断所有旧路径均安全；在建立多图共享、重复显示与释放的实际证据前，不替换 raster 所有权，不放宽 64 MiB 预算与图像校验。独立测试 atlas 不能替代真实 Metal 验证。

R2 基线安全缺陷：独立基线快照用真实临时文件构造本地编辑后外部更改，状态 `ConflictOnDisk`，对关闭标签与退出窗口分别断言应提示；两项均失败（0 passed / 2 failed），表明旧版本会直接关闭。本候选将冲突纳入需要确认的状态，回归与最终复审进行中；此问题为基线既有，不归因于重构。

## 11. R2 完成、集成与交付记录（2026-11-06）

### 完成范围

- R1 四个提交：`96bf2d4` 二进制复用 app lib、`eeeacbc` 抽出 Drawer 搜索输入、`9409eef` 抽出语法共享类型与编辑器 tokenizer、`24ecae5` 抽出预览选区逻辑。
- R2 提交 `7bc6be2`：删除 `AppState.doc` 镜像，活动文档只由 `open_docs[active]` 持有；`sync_from_editor` 不再 clone 整份 `Document`；布局 cache 对未变行移动已有 `String` 而不是 clone；`ConflictOnDisk` 纳入关闭和退出确认。新增 `document_lifecycle_regression_test.rs`、`layout_cache_refactor_test.rs`，把两个曾直接写入私有字段的测试迁入 `state.rs` 模块测试，保留原行为覆盖。
- 原有未提交滚动修复先独立提交为 `69e158b`，再以 `1bd7eb4` 合并 R1/R2；集成时把滚动代码里唯一剩余的 `state.doc.parsed` 字段访问改为 `doc().parsed`，并清理四个新模块的 EOF 空行。未覆盖或重置任何人的改动。

### 判别证据

- 隔离 R2 候选：`cargo test --workspace` **324 passed / 0 failed / 0 ignored**；AGENTS 观测命令 23+5=**28/0**；Phase H 串行 **21/0**。
- 变异：把 `plan_close` / `next_dirty_doc_key` 的冲突判断退回 `== Modified`，两个冲突确认用例 **FAILED**；把布局 cache 命中路径退回 `old.clone()`，文本分配复用用例 **FAILED**。两项均已恢复。
- R2 分配基准：R1 最终版 `24ecae5` 与最终集成版 `1bd7eb4`，同 release、同两份 1.5 MB 文档、每次 35 轮编辑/同步/切换，各 3 次共 105 样本。中位数：`sync_bytes` 4,500,087 → 1,500,021（**−66.7%**），`sync_ns` 292,916 → 219,125（**−25.2%**），`sync_calls` 4 → 1，`switch_bytes` 15,225,464 → 9,225,332（**−39.4%**），`switch_ns` 1,845,000 → 1,647,542（−10.7%）。编辑本身分配不变，未声称稳定 RSS 改善。完整 CSV 在 `/private/tmp/glidemark-r2-baseline-bench` 与 `/private/tmp/glidemark-r2-candidate-bench`。

### 集成后验证

- 最终集成候选：`cargo test --workspace` **329 passed / 0 failed / 0 ignored**（53 行 `test result`）；AGENTS 观测 **28/0**；Phase H 串行 **21/0**；滚动相关 `phase_j_scrollbar_test` **11/0**、`split_scroll_sync_test` **2/0**。
- `./scripts/bench_memory.sh` 最终 release：默认示例峰值 RSS **99.2 MB**、1.5MB 文档 **162.2 MB**。该脚本使用正式配置，与 R0 的 `AppState::new_isolated` 隔离 harness 口径不同，不能相减声称收益；只记录无回退。
- `./scripts/bundle_macos.sh` 成功；`codesign --verify --deep --strict` 通过；安装二进制 16,100,016 B，mtime **2026-11-06 12:12:41**，晚于合并提交 12:11:11。lib/bin 去重后 release 可执行文件较集成前约 20.0 MB 缩小到约 16.1 MB。
- 为避免丢失用户窗口中未保存内容，**没有强制结束正在运行的 GlideMark 进程**（PID 66863）；磁盘上的 app 已替换，运行中的旧进程需用户自行重启后才会加载新二进制。

### 暂缓项与交付边界

- **R3 未实施**：R2 已在编辑同步分配上取得远超探索阈值的收益；后台解析、语法 cache 数据结构和工作区扫描线程化尚无新瓶颈证据，按计划“不为了完成架构全部实施”停止，避免 speculative complexity。
- **R4 未实施**：第 10 节的 Metal atlas 共享/释放证据仍不足，不替换 raster 图片所有权，也不放宽 64 MiB 与颜色校验。
- **R5 部分完成**：仅把受私有字段收紧影响的测试迁入模块并新增回归；当前 52 个集成测试文件、322 个测试属性。未再删除测试，也未声称减少链接耗时；纯算法测试合并留待有同机耗时证据后单独处理。
- 未运行真机像素/滚动验收；本次只做结构、行为、分配和打包验证。仓库工作区除本计划文档提交外保持干净；**尚未 push、尚未更新 PR**，等待用户确认是否按既有授权发布。

## 12. size-table VirtualList：滚动条跟手与分屏底部跳动的根治（2026-11-06）

### 社区结论与本地取舍

- 联网核查 `longbridge/gpui-kit` `gpui-base 0.9.0`：`v_virtual_list` + `Scrollbar` 正是该问题的现成方案。其文档明确把「边滚边测」列为 `Scrollbar jumps / scroll position drifts`，改用预先提供 `Rc<Vec<Size>>` + 前缀和得到精确 offset。
- 但该 crate 依赖 `gpui-pre 0.3.11`，与本项目 `gpui 0.2.2` 不兼容；直接引入等于 GPUI 大版本迁移。按计划不升级、不加依赖，只移植「尺寸表 + 前缀和 + 可见区间」这一决策。
- 网络核查本身可用：crates.io 默认 `curl` UA 返回 403，加 UA 后 200，`cargo search` 与 GitHub raw 均 200；`w3:p9` 的 Codex CLI 沙箱配置显示 `network.enabled=false`，那是宿主沙箱，不是缺插件。

### 实施

- 新增 `crates/glidemark_app/src/views/preview/virtual_list.rs`：`PreviewScrollState` 持有每个 AST 块的估计高度、`origins` 前缀和、`total`、`ScrollHandle`、真实 prepaint 高度表和可见区间。保留 `ListState` 形状的窄兼容方法，使既有行为测试继续通过。
- `render_preview_pane_with_callbacks` 不再用 `ListState`，改为按前缀和只渲染可见块，绝对定位在总高容器内，外层 `overflow_y_scroll().track_scroll()`；真实块高来自块内容 canvas，随帧写回尺寸表。
- 滚动条改为 `ratio = offset / (total - viewport)`：拖动、轨道点击、键盘与窗口外释放都直接使用精确像素范围；不再依赖「已测量项」的 `max_offset_for_scrollbar`，也不调用 `measure_all()`。
- 双向同步滚动改成共享尺寸表：编辑器→预览用 `offset_for_block(block, progress)`，预览→编辑器用 `block_progress_at_offset()` + `SyncScrollMap`；保留 100ms 回声抑制，底部不再把未测块 `scroll_to` 后夹回。
- 尺寸表在手势中保持滑块比例、空闲时保持内容锚点；真实高度跨 AST 重建保留，避免图片/语法回调把预览拉回。可见区间只在视口下方 overdraw，避免把屏幕外文字区域登记进命中表。
- 选区鼠标处理移到整块 item 容器（含 `pb_4`），修复点击落在块间空白时无 anchor。

### 判别证据

- 新增 `crates/glidemark_app/tests/preview_virtual_scroll_test.rs`：
  - 180 块长短混合长文档，拖到 0.31 / 0.68 / 0.97，滑块与指针偏差 ≤2pt，状态比例与目标比例偏差 ≤0.03，且块索引持续前进；旧「边滚边测」实现会在未测区域夹回。
  - 分屏编辑器连续 140 次滚轮到底部，预览偏移不回退，最终比例 >0.85。
- 变异：把 `apply_measured` 强制返回 `false`（退回固定估计、不吸收真实高度），`local_images_test::preview_elements_load_private_snapshots_and_remeasure` FAILED（占位块 y 前后均为 187px）；恢复后通过。
- 全套回归：默认 `cargo test --workspace` **331 passed / 0 failed / 0 ignored**（54 行 `test result`）；AGENTS 观测 23+5=**28/0**；Phase H 串行 8+5+4+4=**21/0**；`phase_j_scrollbar_test` **11/0**；`phase_h_b3_autoscroll_real_events` **4/0**；`local_images_test` **6/0**。
- 打包与签名通过；磁盘 app 已替换。真机触控板、窗口外拖动的像素级手感仍需用户实测，headless 测试不能替代。

### 边界

- 仍是 GPUI 0.2.2、两个 crate；未引入 `gpui-kit`、Rope、事件总线或统一任务框架。
- 未 push、未更新 PR；按用户此前选择仅本地交付。运行中的旧进程需重启后加载新二进制，未强制结束以免丢失窗口内容。
