# 主题选中色恢复与内存审视

版本 R9，2026-11-17（补齐浏览后的资源回落、Rust 1.100.0 升级，以及面向 200MiB 目标的媒体解码与纹理优化；实现于 11 月 16 日，安装采样于 11 月 17 日）。用户已确认：Default 保留当前加深暖灰；其他主题恢复 6dfdab3 的主题适配（accent 混合 surface，浅色 12%、深色 20%）；正文系统原生选区不变。R2/R3 的内存范围为分析和测量方案；R4 已由用户扩展授权为工程落地及本机操作验证。无新增架构决策。

## 实施和验收

只修改 ThemeColors::control_selection_bg 的分支。Default（含 resolve 的旧 miaoyan 别名）保持当前算法；其余 palette.id 明暗均恢复历史公式。共用侧栏/顶栏/设置/查找调用点自动生效，不逐处硬编码。复用已有调色板/原生选区回归用例；这次简单色值恢复不新增镜像实现的测试。本机按约定不运行测试，构建安装并检查签名；不声称全主题视觉验收通过。不推送。

GLOSSARY.md Selection Token 定义应更正为：Default 暖灰、其他主题各自配色，独立于正文原生选区。

## 内存审视：已确认与未确认

用户报告约 400MB。尝试采样 PID 35319 时该进程已退出，ps/vmmap 无有效样本，因此没有复现、没有泄漏结论、没有已测节省数字。Activity Monitor 的 Memory、RSS 和物理 footprint 不混用。以下为代码证据与候选措施。

1. **优先：补齐 Mermaid 超预算回收时机。** preview.rs DiagramCache::evict_overflow 只由 insert_sized 调用；当本帧所有图表被保护时允许超预算，随后离开视口、切 Source 或切文档时，begin_frame 仅加计数而未重新淘汰。无需插入新图即可持续保留超过 32MiB 的条目。建议在本帧可见图表完成标记后做淘汰，并下一帧通过现有 release_evicted_diagram_images 释放资产和纹理；不可在可见标记之前直接清空，以免每帧重载。若改在下一次 begin_frame 处理，必须在递增前依据上一帧 used_frame 保护图表，不能递增后把全部条目当离屏；两种时机选一，不同时叠加。需覆盖同时可见超限、滚离、切 Source、灯箱持有和快速换文档场景。此项是生命周期缺口，不是已观测泄漏。
2. **优先测量：图片与图表各有预算，但没有总媒体预算。** local_images.rs 预览 CPU 图片上限 48MiB（另有 GPU 副本），图表 CPU 估算上限 32MiB（另有 GPU 副本、可见保护允许超限）。纹理会共存，单次解码还有原图/缩图临时内存；这些预算不等于应用总内存。建议先统计两类实际驻留与加载峰值，若占主导再以共享预算协调，优先释放离屏资源，不直接降低清晰度或把可见图片变成错误。48+32 不是实际占用测量，更不能简单等同系统显示值。
3. **后续测量：非活动文档的文本仍保留。** state.rs load_file 保存多个 OpenDoc，旧文档只 unload_parsed，正文仍驻留（OpenDoc.doc.buffer 是 String；core document.rs:204 只替换 parsed）。建议记录文档数量及文本总字节，只有大文件/多文档负载确认占主导后，才讨论干净非活动文档可重载缓存。不得丢弃未保存内容、光标状态或撤销历史；不能本轮暗改关闭/移除语义。
4. **优先测量：灯箱独立纹理峰值。** workspace.rs:2187 调用 core mermaid.rs:1108 的 calculate_hidpi_dimensions，最多 4000px/16MP，RGBA CPU 像素理论上限约 64MB（十进制），另有 GPU 纹理；与预览纹理可能共存。现有 close_lightbox/on_document_swapped 会释放独占纹理，故先测“打开→关闭→空闲”的峰值和回落。若证实主导，再评估按视口和缩放分档加载的收益/清晰度代价；本轮不降低清晰度、不做实现。当前代码是 4000px，非历史实现的 4096px。
5. **较低优先：语法定义与正则为进程级常驻。** syntax.rs 的 SYNTAX_SET（core syntax.rs:202）和 WARMED_LANGUAGES（:256）使用 OnceLock，已按需编译；4MiB SyntaxCache 只覆盖 token 结果，不代表全部高亮开销。先比较纯文本、单代码语言、多语言访问后的增量，不更换成熟高亮库、不把缓存清零当解决。

已存在保护：位图缩到 1600px、图片串行加载、离屏图片驱逐、旧文档卸载解析结果、token LRU、灯箱关闭释放。无需重复实现这些机制。最小可取方向是复用现有缓存与释放函数补生命周期，而非新增通用缓存框架。

## 采样方案（后续执行）

固定同一 release、窗口尺寸、缩放、文档与滚动路径，分别记录启动空白、纯文本、代码、多图/多 Mermaid、灯箱打开/关闭、切 Source、切文档、明暗主题切换后空闲的 RSS/footprint。对照冷启动和重复访问 5 次，区分稳定缓存平台、瞬时峰值和每轮增长。同步记录 CPU媒体字节、条目数、释放队列、打开文档文本字节；GPU/框架/分配器余量单独解释。优化前后对同场景比较，不承诺 400MB 一定降到某个值。Windows 本轮无实机证据。

## 审查记录

用户明确确认上述产品范围；内存不在本轮实施。

R1 审查意见处理：补灯箱独立峰值及明暗主题残留的测量；明确 frame 递增前后的保护条件；正文驻留及 OnceLock 事实补源码引用；CPU/GPU 预算保持为理论上限估算，绝不称实测。与本轮无关的预览徽章事件问题另列后续，不纳入修改。

R2 自查完成：用户于 2026-11-16 明确覆盖独立代理审查关口，要求当前代理自行审查、不再向其他 pane 派任务。t14 的 R1 意见已处理，R2 审查被限流；t13 复核未完成，已按用户要求停止。两者均不记为 R2 独立批准。已自查历史公式、Default 别名、文字原生选区隔离和内存证据边界，按用户确认的范围实施。

## 本轮结果

主题分支已实现；release 构建安装成功（hc t3m），安装脚本签名验证通过。未运行测试（按用户要求），未完成所有主题的逐个视觉验收；不报告测试通过。本轮没有修改任何内存行为，后续优先复现媒体峰值并修补已确认的 Mermaid 回收时机。

协作政策同步到全局 /Users/apple/.codex/AGENTS.md、项目 AGENTS.md、docs/agent-workflow.md，并在相关历史计划顶部加现行约定提示；不重写历史审查结果。


## R3：社区调研与新增节省方向

调研日期 2026-11-16；本地代码基线 `57449cb`，依赖以 Cargo.lock 为准：`gpui-pre 0.3.12`、`gpui-pre-apple 0.3.12`。仅扩充报告，未修改生产代码、未安装依赖、未更换分配器、未运行测试/构建或实际内存剖析。仍无有效的 400MB 现场样本。下文的“代码确认”表示可从实现推导，不等于已经测出该路径贡献多少 MB。

### 社区线索如何取舍

| 一手资料 | 社区结论 | 对本项目的判断 |
| --- | --- | --- |
| [GPUI 图片缓存问题 #27414](https://github.com/zed-industries/zed/issues/27414) 与 [原生 ImageCache #27774](https://github.com/zed-industries/zed/pull/27774) | 切换图片后，资产缓存可能继续持有旧图；上游增加显式管理图片生命周期的组件。 | 优先核对资源所有者与释放链，而非只删临时 SVG。项目已有 `remove_asset`/`drop_image`，应补遗漏时机。 |
| [归还 atlas tile 空间 #58874](https://github.com/zed-industries/zed/pull/58874) | 旧实现移除 tile 时未归还 allocator 的矩形空间，反复加载/释放仍会增长。 | **本地 Metal 依赖已有 `allocator.deallocate(tile.tile_id.into())` 和相应回归用例**，不能重复移植，也不能据此认定当前版本有同一 bug。 |
| [图片重载释放旧纹理 #58803](https://github.com/zed-industries/zed/pull/58803) | 重新解码文件前，跟踪并释放窗口中原先显示的 RenderImage，避免每次重载累积。 | 本地应覆盖刷新、换文档、灯箱关闭、异步旧结果四条路径；现有实现已有大部分清理，先测重复操作后是否回到平台值。 |
| [Windows remove_asset 报告 #56667](https://github.com/zed-industries/zed/issues/56667) | 报告者在 GPUI 0.2.2/Windows 观察到资产移除与 atlas 资源释放不同步；该 issue 关闭为 not planned，带 needs repro。 | 仅作 Windows 专项复现线索，不当成已确认上游修复或本机 macOS 根因。 |
| [CPU 图像副本讨论 #56737](https://github.com/zed-industries/zed/discussions/56737) | 作者在 Windows 私有 fork 尝试上传后不保留 CPU 像素，并报告较大收益。 | 这是单一作者、旧版本及 Windows 结果；**不移植其“300MB→12MB”数字，也不直接采用 fork**。图像重传、跨窗口、设备恢复仍可能需要 CPU 数据。 |
| [Zed Rope/SumTree](https://zed.dev/blog/zed-decoded-rope-sumtree) | Zed 用树形文本结构处理大文档与编辑。 | 可借鉴结构共享和局部更新；ZMark 先减少具体全文副本。直接替换整个文本引擎风险较高，不能承诺换 Rope 就省内存。 |

这里使用上游 issue、PR、维护者文章作为依据；社区个案只用于形成复现实验，不等同本项目诊断。没有采用搜索结果里的第三方镜像作为最终技术依据。

### 新增候选 A：灯箱按实际需求分配像素，不主动填满预算

**代码确认，优先级高。** `crates/zmark_core/src/mermaid.rs::calculate_hidpi_dimensions` 使用面积/边长预算计算比例，但没有将比例限制在“实际视口×缩放”需求以内；`workspace.rs::open_lightbox` 打开时调用它。预算是上限，却也会变成小图的放大目标。

可手算的边界例：输入 500×500、scale_factor=1，现公式的 `final_ratio=4`，输出逻辑 2000×2000，经固定 2× SVG 解码得到 4000×4000，RGBA 为 **64,000,000 B**。若初始显示只需逻辑 500×500，则 1000×1000 物理像素是 **4,000,000 B**。两者相差 60MB 是此假设下的 CPU 像素算术差，不是对当前会话节省量的测量，GPU/系统 footprint 更不能直接按此相减。

最小方向：初始按视口和显示密度确定目标，继续保留现有 16MP/4000px 硬上限；用户放大超过已载清晰度阈值时，再复用现有缩放桶生成高清图。需保证原图自然尺寸、手动缩放中心、缩小后复用和快速关闭时取消加载。不要简单 `min(1)` 后就宣布完成，否则会损伤高 DPI/放大阅读。验收比较相同图在 1×/2×/3× 显示密度、初始与放大后的像素数、字迹、峰值、关闭后回落。

### 新增候选 B：把解码峰值与长期缓存分开限制

**代码确认存在峰值来源，收益待测，中优先级。** `local_images.rs::load_image` 在 `DynamicImage::from_decoder` 完整解码后，再进行 1600px 缩放；压缩输入、完整原图和缩图缓冲在部分阶段可能同时存在。`MAX_CACHE_BYTES=48MiB` 只限制完成后加入缓存的图像，不覆盖解码中的临时内存。当前已串行加载，不能再把“加串行”列成新优化。

优先改进方向是把预计原始像素与缩图临时空间纳入加载前的峰值账本；检查当前 codec 能否在解码阶段降采样、或避免不必要的颜色格式副本，再决定采用什么原生 API。本次检查的 `image 0.25.x` JPEG decoder 没有核实到可直接调用的 `scale` 方法，不照搬旧版 image 或 Dart image 的同名 API。PNG/JPEG/WebP/GIF 需分开分析。保留尺寸/输入限制、首帧策略和 EXIF 方向处理。

### 新增候选 C：全文撤销快照改成局部编辑记录，但不先动上限

**代码确认，中低优先级。** `editor_engine.rs:337–360` 每次 `push_snapshot()` 调 `to_text()` 生成全文 String；撤销和重做分别受 100 步/8MiB 预算限制。正常文本常驻已被限制，不能把它说成无界历史泄漏；但大文件连续局部编辑仍会制造全文复制和分配峰值。

先测“1MiB 文档连续输入/撤销/重做”的分配总量和高水位；收益足够才用可合并的局部替换记录（旧文本/新文本/光标）替代普通输入的全文快照，复杂批量编辑保留现有安全路径。IME、列选择、粘贴、换行合并和保存后的撤销必须保持语义。不要为省少量有上限的内存直接砍撤销步数。

### 新增候选 D：单一文本来源和短生命周期快照

**代码确认存在多种表示，收益待测，中优先级。** 活动文档有 `Document.buffer: String`，编辑器有 `Arc<Vec<String>>`，撤销快照有 String；后台解析/查找等又可能短暂拥有自己的源码表示。`Arc::clone` 本身不复制文本，不能对每处 clone 机械改造；应定位 `to_text()`、实际 String 克隆、`Arc::make_mut` 触发复制的现场。

最小方向：先让只读消费者共享同一不可变修订快照，并确保过期任务/回调不延长旧修订存活。仅当“大文件+小编辑”证明整块副本占主导，再评估局部修订或成熟 Rope 组件。现有行布局缓存已用 hash/len 代替全文行拷贝，这项优化不要重复做。参考 [Zed 的文本结构经验](https://zed.dev/blog/zed-decoded-rope-sumtree)，但新树结构的节点与索引也有成本。

### 新增候选 E：原生图片缓存可用，但采用 RetainAll 不是内存优化

**依赖核实，优先避免误改。** 本地 `gpui-pre 0.3.12/src/elements/image_cache.rs` 有 `image_cache`、`ImageCache`、`RetainAllImageCache`；后者在实体释放时调用 `drop_image`，但它在存活期间会保留访问过的图片。上游 [#27774](https://github.com/zed-industries/zed/pull/27774) 的历史构造方法不保证与本地 API 一致。

因此不把应用根容器换成 RetainAll 来替代现有离屏驱逐。若能缩短对象所有权并简化代码，可在文档/窗口作用域复用原生接口；应用特有的字节预算、输入校验和灯箱持有保护仍需保留。优先复用当前释放函数，不同时叠加两套缓存所有者。

### 新增候选 F：确认 GPU/字形图集与 Rust 堆各占多少

**待测，避免误诊。** 即使 Rust 堆对象已释放，系统显示值也未必立即按同样幅度下降；Metal 纹理、atlas 页、驱动资源和分配器保留空间需要分别观察。依赖修复归还 tile 空间也不代表所有仍有字形引用的图集页立即消失。不能用任意 `shrink_to_fit()` 或替换 jemalloc/mimalloc 来承诺回收 GPU 内存。

采用 Apple 的 [Metal 内存分析](https://developer.apple.com/documentation/xcode/analyzing-the-memory-usage-of-your-metal-app)，同时看 Allocations 与 Metal Resource Events 的分配/释放时间；与 [Rust Performance Book](https://nnethercote.github.io/perf-book/heap-allocations.html) 建议的热点分配分析互补。可用隔离的 [dhat-rs](https://docs.rs/dhat/latest/dhat/) 分析 Rust 全局分配器路径，但它是实验性工具，不把堆数据当成 GPU 或进程全部 footprint，不默认装进用户常用 release。若工具不可用，先记录现有缓存计数和系统采样，不要求关闭系统保护。

### 推荐顺序与可验收实验

| 顺序 | 实验/候选 | 观察信号 | 最小下一步 | 暂不做 |
| --- | --- | --- | --- | --- |
| 1 | 同一组 Mermaid 来回滚动/切 Source 5 轮 | 已离屏超预算条目是否仍驻留 | 补原报告的帧尾回收时机及既有资产释放 | 不砍可见缓存、不重复解码可见图 |
| 2 | 小图与大图各开灯箱、放大、关闭 5 轮 | 请求物理像素、CPU媒体字节、Metal资源、关闭后平台值 | 评估按视口渐进高清，优先去掉无需求的预算填充 | 不直接套社区 fork、不减少放大清晰度 |
| 3 | 单张接近上限的 PNG/JPEG，冷启动加载 | 解码阶段峰值与完成后缓存差 | 为在途解码记账、核实 codec 原生能力 | 不把完成后 48MiB 当总峰值上限 |
| 4 | 大文本连续输入/撤销/查找 | 全文快照和 to_text 分配热点、旧修订存活 | 先削减实测热点，再决定编辑增量记录 | 不先重写整个编辑器 |
| 5 | 同一文档开关/明暗主题切换 | OpenDoc文本、两种图表变体、缓存平台值 | 有证据再收缩非活动缓存 | 不丢未保存内容或用户历史 |
| 6 | 大量不同 CJK/emoji 但无图片的文本 | Rust堆稳定而字形/Metal页增长，或相反 | 分离框架图集与应用堆，核对平台特定实现 | 不将 Windows 专项问题当 macOS 根因 |

每个实验分别记录“进入前、峰值、操作结束、空闲后”及重复轮次。以同一硬件、窗口尺寸、release、固定文档比较；区分 CPU allocation bytes、GPU资源估计、RSS 和 footprint。先修确认的生命周期缺口、再处理过度分配、最后才做数据结构或分配器替换。

### 自查结论与边界

本轮新增最有价值的可执行候选是 **A 灯箱按需分辨率**；新增需要测量验证的是 **B 解码峰值、C 全文撤销快照、D 活动文本副本、F 图集/堆分离**。现有原生资产接口可复用，旧 atlas tile 修复已包含，均无需重新造轮子。所有性能数字均为代码预算/手算或明确标注的外部个案，没有新增实测节省值。

本次自行审查，未向其他 pane 派任务。只修改本报告；文档链接、代码依据与具体适用版本已核对，无需构建安装。


## R4：工程落地第一批

用户已授权工程实施与本机多文档浏览检查，覆盖此前“仅分析、不改内存行为”的范围限制。按当前约定自行审查，不向其他 pane 派任务。

### 已实施并安装

- `DiagramCache::finish_frame` 负责超预算淘汰；通过 GPUI `Window::on_next_frame` 在本帧绘制完成后执行，此时 RenderOnce 才已标记完所有可见图表。淘汰不再依赖插入新图，也不在可见标记之前抢先删除；沿用下一帧的资产/atlas 释放链。
- 灯箱持有的源 SVG 在帧尾显式保护，支持离开预览视口后继续缩放重绘。可见内容仍允许临时超过缓存预算，不通过丢图达到上限。
- 初始灯箱备用 SVG 使用适配后的视口尺寸和屏幕密度，不填满硬预算；复用现有 120ms 防抖清晰重绘。视口适配先于清晰目标计算，避免先以自然尺寸触发一次无用高清分配。
- 清晰重绘成功后释放独占备用 SVG 的资产和 atlas 纹理，不同时长期驻留两份高清纹理。只释放独占资源，保留共享预览源；失败时仍显示备用图。旧纹理保留到新图就绪，避免空白切换。
- 核心 HiDPI 尺寸计算将屏幕密度转换成 GPUI 固定 2× 解码所需尺寸，并按需求而非预算上限放大。清晰纹理的 64px 桶取整后再次硬限 4000px，保持不超过 16MP。
- 旧灯箱磁盘缓存增加 v3 命名，并核对命中项尺寸是否符合当前视口；不误用旧大纹理。

未实施：全文撤销/文本引擎重写、换分配器、共享媒体预算、codec 特定降采样。它们需要对应负载的剖析证据，不因缺少窗口访问而盲目改动。

### 验证及有效采样

- `./scripts/bundle_macos.sh --install` 成功（hc t3n），安装 `/Applications/ZMark.app`，现有 33 条构建 warning，无编译错误；未推送。
- 新增/更新回归用例涵盖：图表离屏且没有新插入时回收、跨多帧可见超预算不反复渲染、灯箱源的 pin/unpin、小图随 DPI 分配而不填满上限、清晰图替换备用图后的释放、严格像素预算。**测试未运行（按用户要求），不能声称回归通过。**
- 实机进程 PID 40266，2026-11-16 17:25:15 采样：RSS 133,712KiB（约 130.6MiB），`vmmap` Physical footprint `81.2M`、peak `87.5M`。17:21:28 的 RSS 为 133,680KiB。此间未能取得窗口操作，不能标为“多文档浏览后稳定”或优化前后对照。
- 起始 vmmap 的 Malloc zone bytes allocated 约 `25.6M`、resident `36.3M`；IOSurface resident 约 `29.8M`。vmmap 的总映射驻留包含共享库，不能把表中 TOTAL `1.7G` 当作进程独占内存。原始采样暂存在 `/tmp/zmark-memory-session/`（临时目录不入 git）。
- 桌面工具开始曾返回窗口 AX 状态，随后截图、点击、快捷键及重新绑定连续报 `cgWindowNotFound`；活动监视器也同样失败。刷新工具会话后仍失败。因此**多开 Markdown、滚动、切换、打开/关闭灯箱的真实操作尚未完成**，400MB 场景未复现，也没有“已降低 X MB”的实测结论。待窗口可访问后，继续表中 1–6 场景，记录操作前后而不是仅起始值。

补充样本：同一 PID 在 18:04:20 的 RSS 为 147,200KiB（143.75MiB），footprint `95.9M`、peak `102.3M`；IOSurface resident `44.7M`，Malloc allocated `27.5M`。这是间隔样本，不是受控的浏览场景；无法确认期间用户操作，不能据此宣称存在泄漏或优化无效。后续需继续分离 IOSurface/窗口缓冲与堆对象的变化。


## R5：真实浏览现场与高亮后端优化

本节更新 R3/R4 的历史证据边界：用户改为自行滚动、预览，代理只从后台采样，因此现已捕获接近 400MB 的现场。早期“未复现”仅描述当时状态，不再是当前结论。仍未取得可自动重复的窗口操作序列，不能称严格的整应用 A/B。

### 旧后端的实机现场

同一 PID 40266，已安装 R4 媒体优化，尚未切换高亮后端。2026-11-16 18:31–18:34 用户浏览期间：

| 时点 | RSS（KiB） | vmmap footprint | 说明 |
| --- | ---: | --- | --- |
| 18:31:13 | 403,888 | 375.8M，peak 389.2M | 浏览现场，RSS 约 394.4MiB |
| 18:33:29 | 415,680 | 390.5M，peak 399.5M | 采样期间 RSS 最大 419,040KiB，约 409.2MiB |
| 18:34:21 | 418,368 | 385.5M，peak 399.5M | CPU 已降至空闲，内存仍驻留 |
| 19:11:12 | 405,824 | 386.8M，peak 399.5M | 安装前后续样本 |

现场 Malloc Small dirty 从约 172.9M 升至 189.8M；18:34 的 IOAccelerator graphics resident 约 57.6M，IOSurface resident 74.7M。不能把这些区域简单相加当作可回收独占内存。`heap -s --noContent` 汇总总分配 191,638,596 B，其中未识别为 ObjC/Swift 对象的分配 183,300,246 B；后者包含 Rust、C 与其他原生分配，**不等于全是 Rust 泄漏**。12 秒 `sample` 采到空闲路径，没有足够热点证据，不据此归因 CPU 调用栈。

原始 ps/vmmap/heap/sample 记录保存在 `/tmp/zmark-memory-session/`，未加入 git；临时文件会随系统清理丢失。RSS 与 vmmap 的 `M` 按各工具原口径记录，不混称 Activity Monitor Memory。

### 定向实验：相同文档与 Syntect 库，不同正则后端

沿用现有 [Syntect 5.3](https://docs.rs/syntect/5.3.0/syntect/) 和 two-face 语法集，比较其支持的 fancy-regex 与 Oniguruma 后端。临时独立进程读取两份实际 Markdown（246,661 B/165 个代码块，以及 51,402 B/27 个代码块），依次预热 SQL、Shell、YAML、Python、JavaScript、JSON，解析所有块并丢弃 token 结果。两种均使用同一探针逻辑、`opt-level="s"`，不包含 GPUI、图片或 GPU。

| 后端 | 两份文档处理后 RSS |
| --- | ---: |
| fancy-regex | 74,288KiB（约 72.55MiB） |
| Oniguruma | 14,320KiB（约 13.98MiB） |
| 差值 | 59,968KiB（约 58.56MiB） |

这是各一次的隔离进程观测，证明此负载存在可明显减少的高亮驻留开销；不是整应用降低 58.56MiB 的保证，也不能称应用从 400MB 降到 14MB。没有多轮统计或完整延迟对比；依赖各后端使用对应的预编译语法数据。探针及构建输出分别在临时目录的 `syntax_profile.rs`、`onig-profile.txt`、`fancy-control.txt`。

### 已实施与自查

- macOS 在现有 Syntect 中启用 `regex-onig`，two-face 配套启用 `syntect-onig`。Windows/Linux 保持原来的纯 Rust `regex-fancy`，不把未经实机验证的平台切换一起带入。
- 原生 Oniguruma 的捕获区域不实现 Send。源码文件的可恢复解析状态因此由每个 workspace 懒创建的标准库线程持有；通道只传不可变代码块、取消标记及拥有所有权的 token 结果。没有 `unsafe impl Send`，没有在 UI 线程等待解析。Markdown 的独立代码块仍走既有后台批处理。
- 追踪 `run_syntax_batch` 调用链，既有调度同一 workspace 只保留一个活动任务及一个待处理请求；原生工作线程不会因每次滚动而新增。workspace 及在途任务释放发送端后，线程接收循环自然结束。取消和旧修订结果丢弃机制沿用现有实现。
- 自查覆盖线程所有权、取消、断开通道、初始化失败、源码跨块多行状态，以及跨平台依赖图。未发现额外值得抽象或合并的重复实现；复用标准库通道，不新增线程池或自制高亮器。原生后端新增 C 编译依赖，仅限 macOS。
- 补充 macOS 回归用例：由不同调用线程请求长 Rust 注释的连续源码块，验证跨块高亮状态，并验证取消请求不产生结果。**用例未运行（按用户要求），不报告测试通过。**

### 安装及新版观察

`./scripts/bundle_macos.sh --install` 已成功（hc t3z，release 构建 22.88 秒），安装到 `/Applications/ZMark.app`。签名验证及已安装/暂存 bundle 可执行文件比较均通过。现有 33 条 app 构建 warning；未推送。依赖图检查确认 Windows/Linux 没有引入 Oniguruma；没有执行跨平台编译。

新版 PID 47505，19:23:01 起始 RSS 124,752KiB（约 121.8MiB），footprint 72.7M、peak 81.2M。这是重新启动后的起始样本，**不能与旧进程充分浏览后的 400MB 直接相减**。新版用户浏览后的峰值仍需采集。下一步继续同一文档、相近窗口和滚动/预览路径，分别观察堆与图形内存；不以空白启动值宣布问题已解决。

19:25–19:28 的第二轮连续采样仍在 RSS 约 121–122MiB、footprint 71.5–72.7M，CPU 大多空闲。未捕获明确的新版文档浏览负载，故该段只作为空闲驻留记录，浏览后验收仍待用户操作。


## R6：新版占用大户与下一批优化价值

代码基线 `1093c4b`。本轮只做现场采样、隔离性能探针和代码审视，不改生产行为、不构建安装、不运行测试、不推送；按用户偏好自行审查，不派发其他代理。工作区出现的未跟踪 `luna.svg` 不属于本轮改动，保持原样。

### 新版浏览后，仍然是谁在占内存

PID 47505 的后续采样补齐了 R5 的浏览阶段：19:33 期间 CPU 有 13–20% 活动，RSS 一度 312,704KiB（305.4MiB），物理 footprint 历史峰值 317.4M。19:39 空闲后 RSS 290,432KiB（283.6MiB），footprint 251.2M；19:42 后续空闲约 263.6M。这些样本与旧版不是相同的自动重放路径，说明新版仍有较大浏览驻留，不用两次峰值之差冒充确定的优化收益。

| 分类 | 19:39 实测 | 能确认什么 | 不应推断什么 |
| --- | --- | --- | --- |
| 图形资源，IOAccelerator graphics | resident 66.7M，后续约 55.8M | 包含 Metal 纹理、图集及相关缓冲，是当前大头之一 | 不能全算作 Mermaid，也不能全当可回收缓存 |
| IOSurface | resident 74.5M | 显示/共享表面占用较大，纯源码模式仍为 74.5M | 未逐资源归因，不能断言全是多余窗口缓冲 |
| Malloc 区实际分配 | vmmap 总计 69.8M；heap 73,211,332 B（约 69.8MiB） | 存活的堆分配已可与图形资源分别分析 | heap 未识别对象包含 Rust/C/原生数据，不能一概称泄漏 |
| Malloc 碎片及保留页 | zone 约 18.8M dirty+swap fragmentation；另有 Large(empty) 约 9.75MiB resident | 部分显示占用不是仍被业务持有的对象 | 与其他字段存在统计口径/重叠，不能相加成另一个精确总数 |

heap 大块中有单笔约 5.48、4.97、4.72、3.83MiB 的未识别分配，大小与数张现有 SVG 的 RGBA 栅格量级接近。这是媒体位图的线索，缺少分配栈，暂不将其逐笔认定为图片。目录里的 35 个图表临时 SVG 也不等于 35 张解码图仍驻留；磁盘文件总数不当作内存计数。

用户确认只切到纯源码且窗口大小未变后：19:50:43 RSS 279,696KiB、footprint 243.9M；19:51:41 RSS 279,728KiB（273.2MiB）、footprint 237.5M、graphics 55.8M、IOSurface 74.5M。图形资源没有随预览隐藏而清空；代码也确认预算以内的图表仍保留。不能将当前 graphics 的 55.8M 全部归因于此，因为字形、窗口路径纹理也仍需要存在。

### P1：预览隐藏时释放图表解码图和 GPU 纹理，保留 SVG

依据：`preview.rs:1070–1073,1157–1185` 的 DiagramCache 只在超出 16 项/32MiB 时淘汰。R4 已修复超预算离屏回收，但**预算以内的离屏/旧文档图表仍可保留**；切纯源码不改变这项策略。32MiB 是 CPU 位图估算，上传图集还有额外成本，且可见保护可能超过预算。

值得优先实施：将便宜的 SVG 文件/渲染结果与昂贵的解码位图/GPU 纹理的生命周期分开。切纯源码、切文档或闲置后，释放已不可见的解码图；保留 SVG，返回预览只需重新栅格化，不必重跑 Mermaid 布局。复用 `lightbox.rs:92::release_lightbox_image` 现有资产和 atlas 释放链，不引入第二套资源管理器。保护可见图、打开的灯箱和在途旧结果；释放只做一次，避免每帧重建。收益可能是数十 MiB 级，**当前没有实测释放差值，不承诺具体回落值**。

### P1：文件内搜索会将密集结果放大，并在关闭后保留

依据：`workspace/find.rs:10–13,18–42,92–115`。每个匹配存一个 `Range<usize>`（64 位下 16 B），再存一个行号 usize（8 B）；结果数量无上限。关闭搜索只设置 open=false；切文档的 invalidate 只是 `clear()`，仍保留 Vec 容量。

隔离探针直接提取当前 `find_ranges`，使用 2MiB 的连续 ASCII `a` 搜索 `a`：2,097,152 个结果，两数组容量共 **50,331,648 B = 48MiB**。单次进程 RSS 从 3,952KiB 到 38,976KiB；clear 后容量和 RSS 都未降，drop 后该次 RSS 也未立即回落。48MiB 是数组容量，不是 RSS 增量；探针简化了行号为一行，不是 GPUI 整应用复现。原始代码/输出在临时目录 `find_profile.rs/find_profile.txt`。

按现有 32MiB 文本输入上限手算，同样的极端密度可使两数组达到约 **768MiB**，另有全文临时 String、编辑器与 AST。这是允许输入下的容量风险，不是此次普通 Markdown 滚动现场的归因。

最小方向：关闭和文档失效时真正放弃结果数组的大容量，重新打开重新求值；避免每个结果重复保存行号；对高密度搜索采用有界结果窗口并按需前后查找，保留上一项/下一项及明确的结果计数语义，不静默截断功能。先解决对象持有，再测分配器保留页，不承诺 drop 后 RSS 必然马上降低。

### P2：预览图表按实际显示像素栅格化

依据：`preview.rs:1326` 通过 `img(path).max_w_full()` 缩放显示；本地 GPUI `elements/img.rs::ImgResourceLoader` 对 SVG 调 `render_single_frame(...,1.0)`，`svg_renderer.rs` 再固定 2× 栅格化，**没有把实际预览栏宽度传给解码器**。例如 SVG 原逻辑宽 1000pt、实际只显示 400pt，2× 屏幕实际需要约 800px 宽，当前却解码为 2000px；同宽高比下理想像素面积约为当前 16%。这是算术示例，非当前窗口已测收益。

可以复用 GPUI 原生 `SvgRenderer::render_parsed` 的 `SvgSize::ExactSize` 或现有灯箱的按需渲染路径，按显示尺寸×DPR 生成预览图，使用有限尺寸桶复用；保持自然宽高比、硬预算及 1×/2×/3× 的可读性，灯箱继续单独高清。不能只将图片显示尺寸调小而称减少了解码内存，也不能将 R4 已完成的灯箱优化重复计功。

### P2：大位图解码峰值，先用原生缩略图能力做对照

依据：`local_images.rs:139–155` 先完整解码，再 Lanczos3 缩到最大边 1600px，之后转 RGBA。接近 4000×4000 的图可能先产生约 64MB 的完整 RGBA，再与输入/缩图短暂共存；64MB 是像素算术，codec 和中间格式可能不同。当前 48MiB 缓存只约束完成后的对象，已经串行加载，不重复提出“改成串行”。

macOS 可优先对照原生 Image I/O：`CGImageSourceCreateThumbnailAtIndex` 配合 [kCGImageSourceThumbnailMaxPixelSize](https://developer.apple.com/documentation/imageio/kcgimagesourcethumbnailmaxpixelsize) 限制缩略图尺寸，并用 [CreateThumbnailWithTransform](https://developer.apple.com/documentation/imageio/kcgimagesourcecreatethumbnailwithtransform) 保持方向。官方 API 确实支持目标尺寸/方向；具体 codec 是否避免完整解码、能省多少，仍需 JPEG/PNG/WebP 分别测，不据 API 名称承诺。保留输入尺寸校验、EXIF、首帧与错误处理；不在无数据时替换所有平台解码器。

### 框架大块：值得量化，暂不直接 fork

本地 `gpui-pre-apple 0.3.12/metal_atlas.rs:150` 每页 atlas 至少 1024×1024；单色约 1MiB、彩色约 4MiB，部分图集空间空闲不代表整页可释放。现实现已归还 tile，并在一页没有引用时释放纹理，不能把已存在的上游修复重复移植。字形 key 还包含字号、屏幕密度、亚像素位置和 dilation；浏览大量 CJK/emoji 或换字号可能增加多套缓存，需单独量化，当前没有图集页分类计数。

`metal_renderer.rs:508–590` 同时按窗口物理尺寸创建路径中间纹理；Apple GPU 的 MSAA 已使用 Memoryless，不能再将其列为新增优化。窗口 drawable、合成缓冲与中间纹理有必要成本；当前 74.5M IOSurface 不能直接当泄漏处理。只有测得隐藏/不用的纹理或图集长期空洞占主导后，才评估延迟分配、闲置回收或上游变更，不为削减正常窗口缓冲贸然改渲染器。

### 本轮不优先的项目与实施顺序

- 撤销/重做已有各 8MiB/100 步预算；浏览不编辑不会不断创建快照。适合后续输入场景优化，不是本次滚动增长的优先解释。
- SyntaxCache token 预算为 4MiB，高亮正则后端已完成实测优化；不再盲目缩减 token 缓存或换第三套高亮库。
- 非活动文档正文确实保留，但此前探针文档仅约 247KB、51KB。这类大小无法单凭原始文本解释几百 MB；只有大量/大文件实际驻留主导时，才做文档缓存策略或 Rope 改造。
- 侧栏扫描已有全局 2000 项/400 文件预算；不把它当作无限目录展开问题。
- 不对所有 Vec 调 shrink_to_fit，不为约 19M 碎片立即更换分配器；额外拷贝、频繁重分配和 CPU 回退可能得不偿失。

建议顺序：**①预览隐藏时释放昂贵媒体资源；②搜索结果有界且关闭后放弃容量；③预览按实际像素解码；④大图片原生缩略图对照；⑤有归因数据后再碰框架图集/渲染器。** 前三项能复用现有资源释放或原生库接口，无需重写编辑器。每项分别保留可重复场景，按对象字节/图形资源/footprint 三层验收，而不是以一次 RSS 下降判定完成。


## R7：隐藏图表资源与搜索结果优化落地

用户已授权从 R6 结论开始实施。本批完成两个 P1 项；预览按尺寸栅格化、原生图片缩略图和框架渲染器仍是后续项目，不将它们记作已完成。

### 图表资源分层释放

- `DiagramCache` 为每个 SVG 记录其解码图是否已被请求。新增 `take_hidden_images` 只取出本帧未使用、未被灯箱持有的解码图，一次释放后不在每个空闲帧重复处理。
- 切换纯源码后，以及切文档的首个完成帧，复用 `release_lightbox_image` 的 `ImgResourceLoader`/atlas 释放链，清理隐藏图表的 CPU 位图与 GPU 纹理；保留原 SVG 和 Mermaid 布局资产。普通预览滚动仍沿用现有预算，不逐帧驱逐附近图表。
- 清理在 GPUI 本帧绘制完成后执行，等可见图表已标记，再决定谁可释放；灯箱源有显式保护。返回预览时再次请求图像，使用同一个 SVG 文件栅格化，不重新计算 Mermaid 布局。SVG 本身仍受旧条数和保守字节估算预算约束。
- 新增回归用例覆盖当前可见保护、灯箱 pin、隐藏资源只释放一次、SVG 保留、重新请求后可以再次释放。沿用已存在的帧尾超预算及光栅资产回收用例。**用例未运行（按用户要求）。**

### 搜索不再为每个结果保存范围与行号

- 用现有标准库 `str::match_indices` 建立最多 1024 个定位点，密度增加时将步长翻倍；完整结果数量继续统计。点击任意结果、上下项、首尾循环时，从最近定位点按需查找，保留 Unicode 字符偏移、多行、字面匹配和非重叠语义；没有截断匹配总数。
- 定位点在 64 位下为 24 B，最多 24KiB。共享一个不可变全文 String 快照支持任意结果重新定位；不是零文本副本，打开搜索时会额外保留一个活动修订文本。无匹配/空查询不保留该快照；空查询也不调用 editor.to_text。
- 删除完整行号数组，并移除原先每次渲染结果列表复制整个行号 Vec 的路径；结果列表闭包仅克隆 Arc，仍用既有原生 uniform_list 展示可见行。
- 关闭搜索、切换文档时丢弃索引与修订快照；旧绘制闭包若仍持有 Arc，会在该帧结束后释放。新索引替换时旧修订也会释放，不用 clear 保留大容量。
- 新增/沿用回归用例覆盖字面与 Unicode、多行匹配、相邻非重叠匹配、跨多次步长翻倍后的所有位置、2MiB 密集结果的索引容量及失效后的 Arc 生命周期。**用例未运行，不报告测试通过。**

### 相同密集负载的隔离性能复测

临时探针直接提取新 `FindMatches`，同样使用 2MiB 连续 `a` 查找 `a`、rustc -O：

| 指标 | 原实现 | 新实现 |
| --- | --- | --- |
| 完整匹配数 | 2,097,152 | 2,097,152 |
| 结果索引容量 | 48MiB，范围数组+行号数组 | 24KiB，1024 个定位点 |
| 额外保存的检索修订 | 原探针本身保留源文本；生产旧实现刷新时临时生成全文 | 一个 2MiB 全文快照，关闭时放弃 |
| 探针进程 RSS | 38,976KiB | 4,128KiB |

本次单次探针 RSS 差约 34.0MiB，不等于应用必然降低该数。新索引建立约 6.94ms；首项、中项、尾项三次定位约 0/0/6μs，不能据此概括其他文档或完整 UI 的延迟。原始探针/输出为 `/tmp/zmark-memory-session/find_sparse_profile.rs/.txt`。性能探针不是自动回归测试。

取舍：用按需重新扫描替代全部结果常驻；极密集的大文件跳转会扫描相邻定位点间的一段，完整搜索仍在 UI 路径同步遍历文本。若该类输入出现交互延迟，再为既有索引建立加入后台任务与修订取消；本轮不额外引入搜索线程池、第三方库或结果截断。

### 自查、构建和交付边界

已自行追踪 find 的计数、渲染、键盘跳转、关闭与文档失效调用链，以及图表使用标记、帧尾处理、重解码、灯箱 pin、旧资产释放路径；使用 ponytail-review 检查正确性和简化空间。复用既有释放函数、标准库匹配器和原生列表，无新增依赖或独立缓存框架；未发现需追加的重复抽象。

`./scripts/bundle_macos.sh --install` 初次 release 构建成功（26.61 秒），因旧程序运行而安装中止；用户确认保存退出后再次执行成功（hc t46，热构建 0.63 秒）。安装路径 `/Applications/ZMark.app`；签名验证、安装与暂存 bundle 可执行文件比较通过。现有 33 条 app warning，未推送。**自动测试全部未运行（按用户要求），没有通过/失败数量可报。**

新版 PID 52991，20:14:44 起始 RSS 125,056KiB（122.1MiB），footprint 69.4M、peak 79.3M。这是重新启动后的起始值，不是隐藏预览后已节省多少的对照。后台继续采集浏览/切纯源码后的指标；整应用本批收益、切回预览的视觉体验、真实搜索点击与键盘跳转仍待用户操作验收。未跟踪 `luna.svg` 保持原样。


## R8：浏览后回落、剩余内存大户与工具链升级

本轮复核 R7 安装版的真实浏览样本、阅读 pi 已有的 Rust 升级结论并自行审查；没有给其他 pane 派任务。生产内存行为沿用 `860c910`，本轮新增工程改动仅为工具链版本及 CI 同步。未跟踪 `luna.svg`、`pelican_bicycle.svg` 均保持原样。

### R7 安装版确实出现回落，但浏览峰值仍需优化

以下都来自 **Rust 1.99.1 构建的 R7 版，PID 52991**。不能将其归因于随后安装的 Rust 1.100.0，也不能将不同进程的任意两个值直接作为 A/B 收益。

| 时刻与阶段 | RSS | Physical footprint | graphics resident | IOSurface resident | malloc 实际分配 |
| --- | --- | --- | --- | --- | --- |
| 20:17:51，浏览活动中 | 308,080KiB（300.9MiB） | 308.2M，历史 peak 318.3M | 86.8M | 74.5M | 94.7M |
| 20:24:21，浏览后空闲 | 212,368KiB（207.4MiB） | 184.8M | 22.8M | 74.5M | 本时刻未单独摘录 |
| 20:28:12，后续审视 | 218,176KiB（213.1MiB） | 185.2M，历史 peak 318.3M | 22.8M | 74.5M | 41.4M |

采样原件：`/tmp/zmark-memory-session/watch.jsonl`、`watch-2026-11-16T20-17-51.vmmap.txt`、`watch-2026-11-16T20-24-21.vmmap.txt` 和 `upgrade_audit.vmmap.txt`。vmmap 的 M 保留其原始显示单位，RSS 明确换算为 MiB；不同统计口径不相加。

同进程中，308.2M 的活动样本随后降到约 185M，graphics 从 86.8M 降到 22.8M，说明资源并非一直累积不释放。但没有逐次记录用户操作、文档和视口，不能将全部回落逐项归因于 R7，也不能用历史峰值减空闲值宣称确定节省量。峰值仍为 318.3M，与 R6 的 317.4M 不是受控对照；**接下来应优先减少加载时的像素分配和临时副本，而不仅是增加空闲清理。**

20:28:12 的 malloc zone 实际分配 41.4M、dirty 62.4M，报告 fragmentation 21.0M。这部分不能当作仍被业务持有的对象，释放对象也不保证 RSS 立即下降。IOSurface 74.5M 在资源回落后仍存在，尚无逐资源归因；不能将其全称为图表泄漏。当前证据不足以支持替换全局分配器或直接修改 GPUI Metal 渲染器。

### 值得继续实施的三项

1. **优先：预览 SVG 按显示尺寸与 DPR 栅格化。** R6 的 P2 仍有效。`views/preview.rs` 的 `img(path).max_w_full()` 限制展示宽度，未限制 GPUI 加载 SVG 时生成的像素；当前加载器走固定 2× 路径。小预览可持有较大的 CPU 位图与 GPU 纹理。复用 `views/workspace.rs` 灯箱已有的 `SvgRenderer::render_parsed` / `SvgSize::ExactSize`，以可见宽度、DPR 和有限尺寸桶决定栅格尺寸，放大灯箱继续独立取高清源。缓存 key 需包含尺寸/密度，不随滚动逐帧重栅格化。先以同一 SVG、相同窗口对比实际像素、峰值和字体清晰度；400px 预览与 1000px 原栅格的面积差只是理论示例，不是已测收益。
2. **其次：避免大图先完整解码再缩小的峰值。** `local_images.rs:139–155` 先 `DynamicImage::from_decoder`，之后才 resize；4000×4000 RGBA 单份约 64,000,000 B，解码、方向变换、缩图及上传可能短暂共存。现有串行加载和尺寸限制保留。macOS 优先测原生 Image I/O 缩略图 API，使用已有 R6 官方参考；逐格式验证是否真能减少峰值，保留 EXIF、首帧与错误处理。不要把输出尺寸限制误认为所有 codec 都避免完整解码，也不为尚未确认的跨平台收益替换全部解码器。
3. **多文档负载：消除干净文档的重复正文。** 新核实 `zmark_core/src/document.rs:71–72,119`：每个 Document 不仅有 `buffer: String`，还保存完整 `last_saved_content: String`；`from_file` 直接 clone。`unload_parsed:204` 只释放 AST，两份正文均继续驻留。因此 R2/R6 的“非活动文档保留正文”应细化为干净文档也保留两份原始文本。10 份各 8MiB 的干净文档仅这两类文本可达约 160MiB，是容量算术，不是当前进程已测占用。活动编辑器、搜索修订快照和撤销记录另算；外部冲突的 `observed_disk_content` 还可能保留第三份内容。

第三项的最小方向是干净正文与保存基线共享不可变文本，编辑时才分离，或给干净非活动文档设置可重载预算。修改前必须保护撤销到原文后的 dirty 判定、外部修改/冲突基线、保存失败与未保存文档；不能直接清空基线或用普通 hash 替代精确比较。其公共 buffer API 影响面比图像尺寸调整更大，先记录各文档实际字节，再决定共享方式。`handle_disk_change:256` 还会先 clone 一份磁盘文本，在干净分支很快清除；可顺带消除该瞬时副本，但不要将它误称为永久三份驻留。

本轮不新增缓存框架、不改文本引擎、不更换分配器。已有 4MiB token cache、受限撤销栈和 R7 的稀疏搜索索引继续保留；它们不是本次纯浏览负载中最有证据的大头。上述三项是后续实施候选，**尚未修改或验证其内存收益**。

### pi 的升级结论与采纳范围

读取 t13 已有结论后，自查接受“只升级 Rust/Cargo 至 1.100.0”。这不是独立审查批准，也不是 GPUI 依赖升级。旧 Quiet.md 调研中已被后续代码解决的保存问题不重新列为本轮 bug。

官方 [Rust 1.100.0 release notes](https://doc.rust-lang.org/releases.html#version-11000-2026-11-12) 确认发布日期为 **2026-11-12**。本次不改 Edition 2021、Cargo.lock、Syntect/GPUI 等依赖或 release 优化参数。Allocator API 稳定本身不会自动消除全文副本、位图或纹理；本轮不把升级记作内存优化收益。sample PGO 和 unused-dependency lint 可另行评估，当前不为它们新增构建步骤。

已审视升级风险：never type / Infallible 及类型推导、闭包捕获变化；Cargo 中间产物目录布局也变化。现有安装脚本使用最终 `target/release` 可执行文件，本机实际构建及打包成功，不能据此推出所有外部构建工具、所有平台及测试都已兼容。

### 已完成的工具链改动与验证

- 安装 Homebrew `rustup` 1.29.3（keg-only），再安装 `1.100.0-aarch64-apple-darwin`，minimal profile 加 rustfmt。`rustc 1.100.0 (0ef1bc386 2026-11-09)`、`cargo 1.100.0 (3acc79c0c 2026-11-05)` 已实际核对。
- 增加标准 `rust-toolchain.toml`，pin 为 1.100.0；`.github/workflows/build-desktop.yml` 的三平台 job 同步 pin，且将工具链文件加入 push paths。没有派发 CI、推送或修改现有 CI 测试范围。
- 保留 Homebrew Rust 1.99.1 作为回退，不改全局 shell 配置。**本机调用需把 rustup proxy 放在 PATH 前面**；标准工具链文件由 rustup 解释，直接调用旧 Homebrew cargo 不会自动切换。参见 [rustup overrides](https://rust-lang.github.io/rustup/overrides.html)。本轮实际构建命令：

  ```sh
  PATH="/opt/homebrew/opt/rustup/bin:/opt/homebrew/bin:$PATH" ./scripts/bundle_macos.sh --install
  ```

- 首次 Rust 1.100.0 release 构建 1m35s 成功，生成并签名 bundle；因旧 ZMark 运行而拒绝安装。用户保存退出后，hc t4f 热构建 0.72s 并安装到 `/Applications/ZMark.app`，exit 0；`codesign --verify --deep --strict` 和安装文件与暂存 bundle 的可执行文件比较均通过。
- `cargo build --release -p release_tool --locked`（hc t4d）成功，app 构建同时覆盖 core。`cargo nextest --version` 仅确认已安装命令能启动，**没有运行 nextest 测试，也没有验证新目录布局下的全套回归**。
- 自查确认 rustup pin 与 CI 一致，远端 `dtolnay/rust-toolchain` 确实存在 `1.100.0` ref，构建脚本的产物路径可用；未发现值得新增包装层或进一步简化的地方。
- app 保留现有 33 条 warning，以及上游 `block 0.1.6` future-incompatibility 提示；没有放宽 lint 或隐藏警告。Cargo.lock 未改变。
- 新版已重新启动，PID 60631。20:50:37 的启动样本为 RSS 166,352KiB、footprint 170.4M、peak 174.2M；只是启动存活检查，非相同浏览路径对照，**不与 PID 52991 的空闲值比较以宣称升级节省内存**。原件 `rust1100_startup.vmmap.txt`。

**自动测试未运行（按用户要求）；macOS 已构建、安装并启动，Windows/Linux 编译和正式门禁尚未执行。** 准备推送构建更新时，在最终候选上再运行 `scripts/full_gate.sh` 并核实三平台 CI；本轮没有推送或创建 PR。


## R9：面向 200MiB 目标的媒体优化与偶发卡顿审视

用户继续授权本机内存和浏览性能优化，期望朝 200MB 以内推进。以下统一将 RSS 换算为 MiB，并保留 vmmap 的 footprint/peak 原始 M 标记；三者不是同一个指标。200MiB 是工程目标，不是任何文档、窗口、灯箱和缩放状态下的硬性上限。

### 本轮浏览基线及证据边界

对已安装的 Rust 1.100.0 版本 PID 60631，在窗口大小不变的情况下浏览第一份长 Markdown（约 195,468 字、5,200 多行），操作分屏预览滚动。本机 CUA 曾成功连接并操作该窗口；之后再次连接返回 `cgWindowNotFound`，不能把一次连接成功表述为全程 UI 验收。

- 21:01:36：RSS 194,032KiB（约 189.5MiB），footprint 159.6M，进程历史 peak 290.2M。
- 21:04:52 滚动后：RSS 258,480KiB（约 252.4MiB），footprint 214.7M，历史 peak 290.6M；graphics 16.9M、IOSurface 74.5M。此时 Malloc Large 空区仍有约 57.2M resident；不能将空区、纹理和所有进程 RSS 直接相加当作独立占用。
- `/usr/bin/sample` 10 秒、2ms 间隔的活动样本存在 CoreText 排版及大量未符号化 release 地址；没有取得可精确归因的单帧卡顿轨迹。不能据此宣称用户的偶发卡顿已经完全复现或解决。

原件：`/tmp/zmark-memory-session/round2_before.vmmap.txt`、`round2_after_scroll.vmmap.txt`、`round2-active.sample.txt`。这些是本机临时测量，不纳入 git，也不等同可随报告迁移的附件。

### 已实施的两项主要改动

**一、Mermaid 预览按显示宽度和 DPR 栅格化，灯箱保留原 SVG。** 之前预览使用 `img(path)`，GPUI 的 SVG 文件加载器按固有尺寸及固定 2x 生成位图，之后才由布局压缩显示。现在复用 GPUI 自带 `SvgRenderer`，后台生成预览纹理；宽度按实际列宽、嵌套缩进、窗口 scale_factor 计算，按 64px 档位减少微小缩放造成的重建。每张预览纹理最多 4MP、单边 4000px；这是分配边界，不会把小图强行放大到上限。双击灯箱仍使用原 SVG，可独立高分辨率渲染。

为纹理资产记录独立的 16MiB 软预算，优先回收离屏旧尺寸；同时可见的不同尺寸变体均受保护，全部可见时允许暂时超预算，避免滚动过程中不断删除再生成。已有切源码/切文档后的离屏释放也接通新资产；旧的 GPUI 图片/灯箱释放路径继续保留。布局 SVG、纹理资产的预算有重叠语义，不把 32MiB 加 16MiB 当作实际常驻总量。代码：`views/preview/mermaid.rs`、`views/preview.rs`。

**二、macOS 大幅不透明 PNG/JPEG 使用系统 Image I/O 缩略解码。** 原路径先完整解码，再以 Lanczos 缩到 1600px；4000×4000 的 RGBA 中间图本身约 61MiB，转换、缩图和分配器驻留会叠加峰值。现在通过 Image I/O 在解码阶段限制至 1600px，并处理 EXIF 方向；输入仍受原有 20MiB、4000px/16MP 限制。Core Foundation/Graphics 采用已在 lock 中存在的版本，只补直接依赖，不引入另一个图像框架。CFData 不复制输入，所有借用及原生对象均限制在一次解码内，返回自有像素。

透明 PNG 保留原 Rust 解码器以保护 straight-alpha 精度；小图、GIF/WebP、Windows/Linux 路径不变。原生 JPEG 的色彩转换/缩略算法会产生小幅像素差异，这是有意采用系统解码的取舍，不能宣称逐像素等同旧路径。参考 Apple 的 [Image Sources 指南](https://developer.apple.com/library/archive/documentation/GraphicsImaging/Conceptual/ImageIOGuide/imageio_source/ikpg_source.html)及 [WWDC 2018 图片内存建议](https://devstreaming-cdn.apple.com/videos/wwdc/2018/219mybpx95zm9x/219/219_image_and_graphics_best_practices.pdf)。代码：`local_images.rs`、`local_images/native.rs`。

顺带取消预览代码块为“复制”按钮在每次渲染时创建完整 String；闭包共享现有 Arc，仅实际点击时转换为剪贴板文本。这是小幅热路径优化，未单独测出应用内存收益。

### 解码隔离测量

使用相同的合成 4000×4000 四色图片，最终均为 1600×1600；单次独立进程 `/usr/bin/time -l` 测量，原生探针直接引用本轮生产 `native.rs`。这是性能探针，不是 workspace 自动测试，也不覆盖全部图像兼容性。

| 输入及路径 | 最大 RSS | 解码耗时 |
| --- | ---: | ---: |
| JPEG，原完整解码后缩图 | 167.7MiB | 326.84ms |
| JPEG，Image I/O 缩略解码 | 63.4MiB | 221.79ms |
| 不透明 PNG，原完整解码后缩图 | 166.9MiB | 368.93ms |
| 不透明 PNG，Image I/O 缩略解码 | 56.6MiB | 132.81ms |

PNG 四角颜色和次序一致；JPEG 四角有至多约 1 个通道单位的解码差异，次序一致。每条只测一次，耗时受同机负载影响，不能作为稳定延迟承诺。**隔离进程的约 104–110MiB 峰值差，不等同整个 ZMark 每次浏览都减少相同数值。** 本次没有在 macOS 上更换分配器或手工调用全局 trim。

有效原件：`decode-final-jpg-old.txt`、`decode-final-jpg-native.txt`、`decode-final-png-old.txt`、`decode-final-png-native.txt`，均位于 `/tmp/zmark-memory-session/`。先前探针使用陈旧 Rust 产物及 PNG alpha 属性识别失败的日志不作为测量结果。

### 自行审查、验证和仍需测量的部分

当前代理按 ponytail-review 自行审查了调用链、异步资产生命周期、同时可见的尺寸变体、原生对象所有权、像素通道顺序和输入上限；没有派发新 pane 或独立代理，不称为独立审查批准。复用现有 GPUI Asset/纹理释放、Image I/O 及 CF 包装，未增加总缓存框架或可配置策略层；未发现应继续抽象或删除的额外层。

新增尺寸/DPR/分配上限和可见变体预算的单元用例；扩充大 PNG 四角/透明度与 JPEG EXIF 方向用例，并同步旧 Mermaid 测试构造器的宽度字段。**自动测试未运行（按用户要求）**，不能写为这些用例通过。macOS release 构建及 bundle 签名成功；保留 33 条已有 warning 和上游 block future-incompatibility 提示。Windows/Linux 尚未构建验证。

### 2026-11-17 本机安装与有限浏览验收

用户确认已退出旧版后，hc t4w 执行带 rustup PATH 的 `./scripts/bundle_macos.sh --install`，release 构建 29.07 秒，安装成功（exit 0）。`codesign --verify --deep --strict /Applications/ZMark.app` 通过；`cmp` 确认安装可执行文件与本次暂存 bundle 一致。已重新打开 `/Applications/ZMark.app`，新 PID **69817**。本轮没有运行测试、推送、创建 PR 或升级依赖版本。

| 时刻与阶段 | RSS（原始 KiB → MiB） | footprint | 进程历史 footprint peak |
| --- | ---: | ---: | ---: |
| 08:16:55，恢复约 195,468 字长文档分屏 | 168,400 → 164.5 | 156.3M | 170.4M |
| 08:20:08，打开约 34,310 字含图表文档、图表加载阶段 | 177,616 → 173.5 | 155.0M | 170.4M |
| 08:21:17，已确认首张 Mermaid 图完成显示 | 177,552 → 173.4 | 154.9M | 170.4M |
| 08:23:56，对话框及交互尝试后空闲 | 192,112 → 187.6 | 173.9M | 186.5M |

hc t4x 在 08:16:55–08:20:14 每约 3 秒采集一次，共 60 个 RSS 样本，约 **157.3–173.5MiB**；每 5 次补充 vmmap。原件 `r9_live.jsonl`、`r9_live_*.vmmap.txt`、`r9_installed_start.vmmap.txt`、`r9_first_diagram_pending.vmmap.txt`、`r9_first_diagram_loaded.vmmap.txt`、`r9_installed_final.vmmap.txt`，均在 `/tmp/zmark-memory-session/`。最后一项图形区约 23.9M（resident 19.1M）、IOSurface resident 74.5M，后者仍是明显的窗口基线；不能用它们和 footprint 直接做互斥占用加法。

实际 UI 验证：原生打开文件对话框可选取已有 Markdown，标题及内容确认切换成功；预览滚动到第一张 Mermaid 后先显示占位，随后完成图表绘制。曾通过窗口 zoom 恢复和还原尺寸，因此**这不是与旧 PID 60631 完全同路线、同状态的 A/B**，不计算全应用节省百分比。

CUA 对主窗口点击/滚动间歇返回 `noWindowsAvailable`，原生文件对话框后曾暂时恢复，后续操作又出现 `timeoutReached`。灯箱按钮及图表点击未取得“灯箱已打开”的可见结果，不能宣称灯箱或连续多图滚动已验收。超时后的 3 秒 sample 显示主线程停在 AppKit 等待事件，而非忙于重排或互斥锁；这不是完整健康证明，也不足以将自动化接口失败判定为应用卡顿。原件 `r9-ui-timeout.sample.txt`。没有强制退出或丢弃用户内容。

**本轮已采样场景的 RSS 低于 200MiB；尚不能宣称长时间、多图、灯箱或所有瞬时峰值都已低于 200MiB，也没有帧耗时数据证明偶发卡顿完全消失。** 旧进程长文档滚动后的约 252.4MiB 与本轮约 173.4MiB 场景不同，仅作为后续固定浏览路线测量的线索。

后续重点仍是：按相同窗口和浏览路径复测整应用；分离窗口 IOSurface 基线、透明 PNG 的完整解码峰值、灯箱高分辨率纹理，以及长文段 CoreText 排版。多份大文本的保存基线共享继续列为下一阶段，需保护 dirty/冲突/撤销语义。本轮不通过丢弃正文、降低可见内容或清空撤销栈强行达到 200MiB。
