# I1/I2 返工 #4：分阶段补齐已批准契约

现行设计仍是独立签字 **1c9bbd1**，详见 [主规划](phase-i-color-and-media.md) 末节及 R1–R5。本清单只缩小每次交付/审查的范围，不增加新产品选择或重新解释资源预算。阶段 A 独立审过再派 B；不把部分修复称为 I1/I2 整体完成。

状态：阶段 A 最终候选 **38bbf9e** 独立通过（A内P1:0）；阶段 B 首交 **9bed83d** 打回，原报告7项P1经归因更正后为**6项**，按下方补修批次推进。现行设计1c9bbd1/清单16666bf不变，整体 I1/I2 尚未完成。

## 已确认的打回基线

候选 **9ef5304**，独立审查 P1:7/P2:5，证据 `/private/tmp/glidemark-phase-i-i12-rework3-review/REVIEW_REPORT.md`。默认全量精确 **331 passed / 0 failed / 0 ignored**，48 个结果行；Phase H **21/0**（B3/B4 的 8 项与默认重叠），I0 6 项已在默认内；codesign 通过。该统计是当前失败实现的回归基线，不是功能合格证明。

- P1-1：源码仍有独立行扫描器，CRLF/列表/Tab/引用与 canonical 不同，源映射缺失。
- P1-2：生产同时写入 Workspace 和进程 static CACHE，预览还有 global fallback。
- P1-3：内存按 len/固定常量少计，map/deque/retained 多份 key 和容器容量未记账。
- P1-4/5：BudgetExceeded/Cancelled 都伪装成成功 Plain；没有真实原因、预算重试或有效预热。
- P1-6/7：源码可见高度写死 60 行；没有稳定资源准入与资源不足提示。

保留有效项：真实预览小块按需恢复、per-draw armed/集合去重、唯一显式刷新入口、切文档清表、代码单 run 几何和嵌套 AST；避免为重做这些再加一套旁路。

## 阶段 A：一个 canonical 模型、一份结果、真实记账

本次只派 A，目标关闭 P1-1/2/3；P1-4/5/6/7 及其测试缺口在 B 继续处理，不得声称整体验收通过。

1. 在生产 parser 的事件/offset 路径产出同 revision 的代码描述和逐行映射：canonical payload/语言、payload 行范围、source content/前缀范围、EOL 宽度、synthetic LF。适用于顶层及 ListItem.body/BlockQuote 内代码；不在 App 再逐行扫 fence 拼文本。源码只消费描述，映射先校验非 EOL 字节相等及 UTF-8 边界；不等块两侧 Plain 且有可辨错误，不改源文本。**缓冲领先 AST 的 debounce 期间**，描述 revision 不等 editor revision 时，源码暂用无角色的 Plain 呈现，不拿旧 AST 角色套新缓冲、不另起当前 buffer 的 fence 扫描器或 UI 同步 parse；同 revision 描述就绪再恢复。映射行范围同时供 fenced 几何判定消费。
2. 删除 `extract_source_code_blocks` 及其生产调用。源码可见着色、代码几何范围、预览需求取同一描述的内容 key；同一块不再因 CRLF/容器前缀产生两个条目。原文件/保存/源码复制保持，预览代码复制保持 canonical 语义。
3. 删除进程 static CACHE 的生产读写，包括批次双写、preview.or_else(global)、旧 helper 的生产旁路。测试改为实例缓存或纯 parse，不为旧用例保留可被生产调用的隐式全局副本。仅 SyntaxSet/受限预热可进程共享。切文档清空本 Workspace 不应再从其他缓存回填旧角色。
4. A 固定一个运行期内容 key：`CodeKey { language: Option<&'static str>, payload: Arc<str> }`，由 Core 解析描述生成并供两侧/cache/容纳/尝试表共用；language 只取既有 canonical resolver 的固定受支持名称，None 表示无语言/text/未知均为 Plain（原始 info string 另存展示，不进入身份）。相同 language+payload 按内容相等/哈希相等，不能以 Arc 地址或 hash 单独判等；同内容不同围栏只共享一条语义结果。空 payload 保持空并立即 Ready/无区间，不因视觉空格发任务；None 语言同样直接 Plain，不占解析队列。缓存和解析入口消费**已 canonical 的 language**，不得再把 Bash 的显示名称当 raw alias 二次解析。B 直接复用该类型，不另造 `(String,String)` 键。key 仅用于运行期，无需把静态字符串生命周期写入持久化格式。
   缓存、索引、容纳集合中实际保留的 key/角色数据和容器 capacity 均须在 4MiB 额度内记账；同一 Arc allocation 只计一次。不能继续 `len + 64` 冒充实际容量。优先把重复容器压成已有小型具体结构，避免新框架；若 HashMap 不便给可靠容量界，允许采用更简单可核算的单一容器。不把帧临时分配和持久保留混成一项，也不将文档/纯几何数据算作已受 4MiB 限制。
5. 入库返回明确准入/拒绝结果供 B 消费，不能在超额时对调用者静默成功；本阶段维持预算上限，不增加条数上限。预算/取消语义和重试的完整行为仍在 B 验收。

### A 必须逐项消除/迁移的现有依赖（9ef5304 行号）

| 位置 | A 的处理 |
| --- | --- |
| preview.rs:1881 `.or_else(get_cached_code_spans)` | 删除全局回填，仅读本 Workspace 实例 |
| workspace.rs:444 `insert_cached_code_spans` | 删除批次双写，仅本实例入库 |
| editor.rs:471 `tokenize_document_lines` 读全局 | 去掉隐式全局读；必要的纯 Markdown 分词与描述驱动代码着色分开，不借旧 helper 留旁路 |
| editor.rs:378 `tokenize_code_line -> get_or_parse_code_spans` | 删除同步解析/全局写兼容 helper，调用它的旧测试迁移实例或纯 parse |
| editor.rs:390 `extract_source_code_blocks` | 删除第二套 payload 扫描，App 消费 parser 描述及映射 |
| editor.rs:310 `compute_fenced_code_lines`，:1076 几何调用 | 删除第三套 fence 扫描，几何 fenced mask 消费同一描述行范围，引用内代码也走单 run |
| editor.rs:917 `wrap_document_to_visual_layout` | 无生产调用的旧包装，仍经旧分词读全局；删除或改为显式描述/实例输入，不为它保留全局旁路 |
| phase_i_syntax_and_warm_palette_test.rs:3/235/239/265/277/478，visual_line_wrap_and_syntax_test.rs:7-8/192/204 | 迁移全局/旧 helper 调用至实例缓存或纯 parse，保留有效断言，不用 `allow(dead_code)` 保兼容 |

A 暂保留现有 interim 可见窗口（workspace.rs:335 的 `start_vis+60`）供小窗口接线证据使用；**不宣称关闭 P1-6**，真实 bounds 明确在 B。`source_code_blocks` 将迁移为同 revision parser 描述；Test11 的 `Arc::ptr_eq` 不变 revision 复用契约同步保留或等价迁移，不能通过删除复用断言过关。描述落后时的临时 Plain 不授予回扫权限。

### A 的最小独立证据

- 一个经真实 `parse_markdown` 的 12 组映射表，沿已核准 probe 的输入：LF、CRLF、列表空格/Tab/CRLF、引用及引用内列表、无尾 LF、缩进、空/空白/CRLF 空行。验证源切片/前缀/EOL/synthetic LF 与 canonical 投影；普通 Rust 小块的 CRLF/列表/引用另经真实 Workspace draw 验证源码和预览最终颜色。再验证编辑后立即 draw（buffer 领先 AST）没有旧角色错位、回扫或同步 parse，同 revision 描述到达后恢复。
- 用本实例**恰好一个内容条目**和最小生产解析调用计数证明同块两侧至多一次解析；两侧都有颜色不是这个性质的证明。恢复旧 payload 扫描器及恢复 global 双写/回填路径这两类**可编译**定向回退，分别必须使对应检查变红。实例隔离/切文档检查证明去掉 Workspace 中条目不能从其他缓存恢复；正常代码不留生产 global API。测试不得自己预热被观测 Workspace 目标来假装元素接通，也不为变异新增正式全局缓存框架。
- 小型 cache-only 分配/容量检查验证保留内存与 ledger，含 key 共享、容器扩容、淘汰和拒绝。基准用计数分配器（或等价方法）测稳定时刻的**持久条目/容器分配增量**，独立于 ledger 计算；禁止用 len+常量估算再与自身比较。区分持久/帧临时、同 Arc 引用与真实独立副本，核对额度边界附近仍不超限。
- `flatten_copy_test` 改为生产 parse→flatten，保留已批准的四条新期望、三个边界、八条旧兼容。移除产品测试中直接复制的 REVIEW-ONLY `replan4_flatten_ref.rs` / `replan4_compat_probe.rs`，有效期望并入最小生产路径表，不另做一次全仓历史测试清理。
- 反例先在旧生产入口为红，修复后为绿；新 API 不存在导致的编译失败不算行为判别证明。映射/实例/记账的必要接线回退要能红，不扩成全历史变异平台。

## 阶段 B：有原因的有限解析、真实可见窗口、稳定资源降级

待 A 独立审查确认其目标闭合后另行派发：

1. 解析返回可区分 Ready、BudgetExceeded、Cancelled、硬阈值/未知语言/映射/初始化失败的具体结果。取消绝不作为成功 Plain 入库；旧请求归属、doc/revision 分别验证，不污染最新 pending。
2. 代表性多行预热逐语言校验；有限代次0/1、每代每块最多2次，总自动尝试≤4。首次预算耗尽一次后台重试，二次明确原因；同集合 draw 不阻断需要的重试，也不能令失败热循环。只有刷新预览按钮可在同 revision 重置；保存/Cmd+S 等仍不可。
3. 源码只新增实际 list bounds，offset/行高/VisualLine 求可见范围，删 `start_vis+60`。两侧当前帧范围/遮罩去旧记录，同 draw 只一个 defer，相同集合不新开任务，程序滚动/模式/resize都生效。
4. 按已批准稳定阅读次序做字节准入；溢出显示本地化原因和完整原文，不用 HashSet 随机顺序，不静默丢弃，不逐出本轮容纳项循环。下一帧源码/预览同一状态。
5. 替换 Test12 的双批句柄覆盖为 doc/revision 各自单批真实完成，定向回退对应守卫各红；替换 Test14 的750行真实时钟为最小受控中/末/单行预算反例；Test13 以小块表驱动8/17/33/35实际两侧颜色与字节淘汰后恢复，不仅 park 时间。每语言预热、重试原因、取消、容纳与可见高度各保留必要生产证据。

## 每阶段执行与交付

- 至少 A业务/B独立测试两个子代理按文件隔离并行，主执行代理负责 App 接入与整合；记录实际 agent ID/职责/完成状态。每阶段约定接口后推进，不让测试代写另一份算法自证。
- 中间相关测试，阶段最终精确候选全量与 Phase H 21 串行；I0 颜色/预算保持。允许有效测试数下降，精确报告 passed/failed/ignored 和结果行数，禁止“330+”。
- 独立阶段提交与打包/验签，确认包晚于代码提交、工作区干净、子代理全结束。A 交付注明 `阶段 A 完成 / 整体 I1/I2 BLOCKED` 并列出 B 未完成项，协调者只验 A，不提前派 I3 或创建最终 PR。
- 任务完成并通过最终独立验收后由协调端推送创建保持打开的 PR。该授权不意味着每个未验收中间阶段提前发布。

对 0e0f3d7 清单审查回应：补齐全部 global 依赖、第三套几何 fence 扫描及旧测试迁移；加入 AST 落后时 Plain 和立即 draw 证据；固定 A/B 共用运行期 CodeKey 与空/未知语言归一；明确 A 暂留60行窗口且保留描述 Arc 复用，真实高度只在 B 验收；把“一次解析”落实为唯一条目+生产调用计数和两个可编译路由回退红灯，记账采用独立持久分配测量。原1c9bbd1契约、阶段拆分和产品选择不变。此修订仍需独立确认后才派 A。

## 阶段 A 派发签字记录

Reviewer `w3:p7` 核准 **16666bf**，无未解决实质异议，证据 `/private/tmp/glidemark-phase-i-i12-rework4-review/REVIEW_REPORT_16666bf.md`。清单五处精度全部关闭，原设计不变，本次派发不用重复用户确认。

执行时采纳非阻塞补充：上表已补旧 wrap 包装；edit-then-draw 检查中未变的既有代码行继续复用几何、新输入且未有描述的行按非 fenced 几何，不能回扫补色；优先让 AST canonical payload 与 CodeKey 共享 Arc，若保留独立 AST String 分配需如实报告，不把两份说成共享；增加一个 shell/bash 真实小块反例，验证 canonical 库名称不再被 raw alias 二次解析成 Plain。该问题已实证存在，不是仅检查字符串 API 名。

本记录只确认阶段 A 的实施许可，不表示代码已合格。阶段 A 交付/独立审查仍按本清单；整体 I1/I2 在阶段 B 完成及终验前保持未完成。

## 阶段 A 首交打回与小范围补修

候选 **cc861db** 独立审查为阶段 A 打回（A 内 P1:1/P2:5），证据 `/private/tmp/glidemark-phase-i-i12-rework4-stage-a-review/REVIEW_REPORT.md`。Reviewer 已独立验证映射/单实例等生产主体基本成立；默认 **339/0/0，48结果行**，H **21/0**，I0 6项含默认。唯一审查阻断是两条已要求的判别证据未落地：M1 将 preview content_key 置 None、M2 去掉 debounce 的 is_ast_in_sync 门，交付四用例仍全绿。仅 Bash canonical 回退能使现有用例变红。

本次补修加强现有 edit-then-draw 用例：真实编辑并同步 buffer 后、不 park 立即 draw，断言目标代码行单一 CodeText/完整原文、无旧 Keyword/Number/CodeDelimiter；同 revision 描述到达后再断言实际彩色。另补一条 Preview-only 的真实 draw→后台→draw，断言本实例恰一条非 Plain 且实际预览高亮；不得借 Source/Split 提供需求。M2/M1 各自必须使对应检查精确变红，恢复生产实现后通过。不新增参考实现或只断言缓存非空。

协调者源码补核发现：editor.rs:1279 与 preview.rs:1911 的观测调用未受 feature 隔离，obs 模块全局 Vec 每帧保留代码 token/highlights 副本。该长期积累违反 A 的持久着色数据约束，一并改为 **test-observation 下才启用采集**；正常无该 feature 的构建不写这些日志。依赖观测的现有及新增 UI 用例明确用该 feature 运行并单列结果，不能删断言/ignore 后宣称默认已覆盖它们。不改历史测试策略，仅关掉生产观测旁路。

已知 P2 不视作无风险：缩进代码描述的无效/错误呈现、实际堆含舍入超 ledger 约3.9%、insert bool 尚未消费、逐行 get/反复 shrink 的性能、同 revision AST 落地后几何 mask 未刷新。它们在 B 的状态/准入/视口收口时逐项处理并复验，当前不宣称真实堆严格≤4MiB，也**不批准**默认允许4%超额。B 最终准入须符合原预算契约或返回规划讨论，不能沿用 P2 标签静默放宽。

补修交付仍仅供 A 独立复审，整体保持 BLOCKED(B待执行)；最终全量与观测 UI/Phase H 专项、打包/签名、代理结束与精确统计照旧。

阶段 A 补修 #1 候选 **2c9d3d4** 已交付待独立复审：默认报告321/0/0（46结果行），观测目标14+5=19项/H21项另行运行。321相对339下降来自两组目标的 required-features 分组，不是删除18项；本次新增Preview-only检查后，默认与观测目标合计340项。AGENTS.md 已补两组观测目标的明确阶段必跑命令；它们含算法及UI检查，不能全部笼统称为UI，默认全绿也不能替代这些目标。提交12:44:23，二进制12:44:44，签名通过；统计及M1/M2判别力待Reviewer独立复验，P2和整体B待执行状态不变。

阶段 A 补修 #1 独立复审仍打回（A内P1:1）：M1/M2各自精确红、还原5/5绿，生产feature-off无token/highlight日志已确认；新阻断是全局计数分配器测量受并行环境影响，完整默认曾出现316/2、再跑321/0，单目标压力复跑也有失败。证据 `/private/tmp/glidemark-phase-i-i12-rework4-stage-a-fix1-review/REVIEW_REPORT.md`。

**补修 #2 仅做测量隔离。** 协调者核对精确 2c9d3d4 及 cc861db：同文件三个测试本来就都取 TEST_MUTEX（行80/164/345），因此报告“只有测量用例持锁”不是正确的缺锁根因；不重复增加已有锁，也不声称已证明具体外来分配来自何线程。采用 Reviewer 提供的另一最小方案：将计数分配器和现有 cache accounting 用例移到只含该用例的独立测试目标，原 mapping/bash 目标不再安装进程全局分配器。测量前完成必要语法初始化/样本准备，保留真实分配与现有断言，不重写 ledger、不加 sleep/retry 绕过、不放宽误差、不开全套串行掩盖问题。新目标仍由默认 cargo test --workspace 发现，测试总数不因迁移变化，结果行数可能增加一行。

独立目标有限正常/压力复跑和默认全量验证后再提审；若仍失败继续定位测量污染，不报告已修。观测19项/M1/M2结论及生产代码保持，H21/I0和原P2清单继续保留，整体仍BLOCKED(B待执行)。

## 阶段 A 验收签字与阶段 B 派发

Reviewer `w3:p7` 对 **38bbf9e** 明确“阶段 A 通过，A内P1:0”，证据 `/private/tmp/glidemark-phase-i-i12-rework4-stage-a-fix2-review/REVIEW_REPORT.md`。正常目标10/10、外进程调度压力4/4、默认并行全量连续2/2均通过；默认 **321/0/0、47结果行**，观测目标 **19/0**，H **21/0**（8项默认重叠），I0 6项及hidpi8项在默认内。测试数迁移未变，原断言未放宽；M1/M2与生产feature隔离沿用上一候选已通过证据。仅认可有限复跑未再复现，不宣称永不flaky；无真机截图，审查端Metal工具链限制如原报告保留。

按已批准的上述阶段 B 清单现派实现，无新增产品选择：具体解析结果/取消不入库、有限重试与代表性预热、真实editor列表高度、稳定字节准入及本地化原因，以及生产Test12/13/14判别检查。B复用已通过的canonical映射/CodeKey/Workspace实例与已关闭生产日志；不得重新增加扫描器或global fallback。

B还须按已公开P2完成收口：原4MiB预算未放宽，必须用独立归属测量验证，不能只让ledger≤cap；9bed83d后续drop差分已证明当前cache-owned真实字节等于ledger，旧总堆超额归因正式更正，见下方记录，不再要求修不存在的当前cache越界。仍须替换交付用例的同helper自证。消费insert拒绝值与原因、AST几何失效、Indented正确映射、每块lookup及去逐条shrink等按B清单处理；旧锁/冗余cfg/stub等琐碎项不单开新阶段，不以清理为由扩大范围。

独立测试仍与业务子代理并行、文件隔离；正常/预算失败/取消/内存压力的预期必须不同，不能都缓存为成功Plain。相同可见集合可避免重复调度，但必须允许状态机内约定的有限重试和最新pending推进；实际height、后续AST与doc/rev/request归属构成测试的真实触发。若原预算上界或公开API确实无法成立，先报告具体反例及最小替代回规划，不改数值/口径后直接交付。B完成还需独立复审，之后才推进I3与单独测试清理；任务最终独立验收后创建PR。

## 阶段 B 首交打回与补修 #1：先恢复可复现门禁和判别证据

候选 **9bed83d** 独立审查打回（报告P1:7），证据 `/private/tmp/glidemark-phase-i-i12-rework4-stage-b-review/REVIEW_REPORT.md`。默认 **322/0/0，47结果行**；AGENTS 标准观测命令因 app feature 未转发 Core feature 而 E0432 编译失败，额外手动加 Core feature 才有 **23/0**；H **21/0**、I0保持。该额外组合不等于标准门禁已通过。

原报告还列：失败原因未接UI、三个守卫用例被错误request_id遮蔽、retry/pending覆盖、真实缓存边界超额且测量复用生产helper、预热校验缺失、阅读优先级未实现。保留有效成果：typed结果/仅Ready入库、取消不入库、真实budget retry、唯一刷新入口、AST几何mask失效、小块8/17/33/35真实前景及逐出恢复、A回归。未完成项仍是B责任，不因拆批消失。

**补修 #1 只处理已确定的门禁/证据缺口**（不宣称阶段B整体完成）：

1. App 的 `test-observation` 显式转发 `glidemark_core/test-observation`，原样运行 AGENTS 观测命令，不加未记录flag。feature-off无模拟时钟/观测收集器的边界继续保持。
2. 三个 stale 场景分别只让 doc/revision/request_id 中被测维度不同，其余维度匹配当前值。先让正常同归属请求实际入库，再定向回退各守卫，对应检查精确红；不通过句柄覆盖或另一守卫把请求提前否决。
3. 去掉Test14跨线程spin等时序竞态。采用同线程、有限、只在test feature启用的受控预算时钟/行后钩子；块起点与前后预算判定必须消费同一时基，使恢复慢行重置、末行豁免能真红，不能只往elapsed加一个与start_time无关的常数。无sleep/retry转绿、无通用时钟框架。涉及进程级测试hook的用例按现有隔离策略保护，不能污染其他纯解析检查。
4. 独立记账测试不再调用生产 `heap_usable_size` 作为“实测”；macOS参考用原生 `malloc_size(ptr)` 等实际分配。先隔离一次性语法/regex初始化，或用释放cache前后的持久分配差分核对归属；明确cache对象仍持有的分配与drop后残留的语法单例分配。原4MiB预算不变，不以新的相同helper或宽比值自证。生产记账算法是否需要修改，等待下面事实核查，不先根据总堆差值随意加裕量。

**两条归因待Reviewer补证，不自动豁免**：①实际生产创建pending会先递增request counter，旧active guard通常因此拒绝旧结果；若如此，“accepted retry与更早pending在同回调覆盖”可能与现有ID不变量互斥。须给出生产事件/request序列证明可达，不能仅手工写public字段构造不合法状态。②现有 realheap probe 在 `fn init` 后取baseline，再解析数千带数字/字符串的新块，可能把SyntaxSet/fancy-regex新增懒编译持久分配算入缓存；该语法集合在预算范围之外。须增加drop(cache)前后差分、drop后残留与精确预热再测；若实际缓存仍超额继续按原预算修复。两者尚未独立闭合前，不标记问题已修或报告无风险。

失败原因真正显示、逐语言验证/失败处置、焦点/中心稳定容纳和资源原因等生产接线留下一批；真实bounds的800 fallback及现行P2也继续公开。补修 #1 业务/独立测试两子代理文件隔离，最终原样门禁/观测/H/I0/打包，精确报告批次范围与剩项。该拆分是恢复已批准B证据，不改变产品范围，不另做I3或全仓清理。

## 9bed83d 两条审查归因正式更正（不是修复）

Reviewer 已完成协调者请求的独立补证并明确撤回/更正，证据 `/private/tmp/glidemark-phase-i-i12-rework4-stage-b-review/REVIEW_ATTRIBUTION_CORRECTION.md`；旧报告和日志保留，不改写审查历史。

- **原P1-4撤回**：生产pending只有一处赋值，先递增counter再建请求。存在pending时，旧active.id<current，guard拒绝整个结果/retry分支，只启动pending；active被accepted时pending必None，同一同步回调无法共存。所谓“先retry覆盖，再pending取消retry且id失效”的序列不可达，不应修这个不存在的正确性缺陷，也不要求构造该非法状态的测试。实际新请求替代在飞任务会白算/重复解析，改记P2性能取舍，不能称为已修P1。守卫测试互相遮蔽仍是独立缺陷，继续补修。
- **原P1-5的cache越界归因更正**：原生 malloc_size + drop(cache)差分、样本形状预热后同进程两遍，均得到20001条、`cache_owned_real=ledger=4,194,304=cap`，差0；条目3,801,088 + 容器393,216。drop后语法残余6,864/0，原+72KB来自SyntaxSet/fancy-regex懒编译和测量组成，不属cache预算。当前生产记账不需要据此改算法、加额度或加随意裕量。非mac的16字节估算仍不能称实际上界。
- **不豁免记账证据自证**：交付用例以生产heap_usable_size当参考，恒等size变异仍绿，仍须换独立原生分配基准并验证cache释放归属，不能只用旧宽比值证明硬上限。这个测试侧P1保留。

更正后现行6项P1：标准观测门禁feature接线、真实降级原因UI、守卫测试判别力、独立记账证据、逐语言warm校验/失败处置、阅读优先准入。当前补修 #1 正在处理其中门禁/守卫/测量证据及确定性时钟；其余三项生产接线仍待下一批。没有提前验收新候选，不改变预算/产品选择。

## 阶段 B 补修 #1 复审与补修 #2 范围

候选 **1368e4a** 本批打回，两个证据阻断尚未关闭；证据 `/private/tmp/glidemark-phase-i-i12-rework4-stage-b-fix1-review/REVIEW_REPORT.md`。已独立关闭标准 feature 门禁和三维归属守卫：AGENTS 原样观测命令 **18+5=23/0**，doc/revision/request_id 三个定向回退分别只使对应检查变红。默认 **322/0/0、47结果行**，H **21/0**，I0在默认内，包14:30:31且严格验签通过。保留这些成果，不重复改已正确的业务调度。

**补修 #2 仅补齐以下两项，不扩展下一批生产接线：**

1. 当前 mock 与生产分别用 `mock_start_ms`/`start_time`，恢复生产慢行重置仍绿，不能证明累计预算。收敛成一个最小 `now_ms()` 接缝：块开始只取一次 `start_ms`，行前/正则返回后的两个检查统一 `now_ms().saturating_sub(start_ms)`；仅取当前时刻的底层在 test feature 可注入。不得保留两套 elapsed 判定或引入 Clock trait/框架。受控中/末/单行检查继续覆盖共同生产路径；将旧慢行重置施加到这个共同起点后必须红，不能只修改测试专用时钟让它红。用小型 Drop 清理保障 panic 后恢复测试时钟，避免后续用例受残留污染。
2. 记账基准虽已改为独立 `malloc_size` 和释放差分，单条样本及64B容差仍无法发现恒等size helper。复用现有独立 allocator 目标，增加有界的小预算填满检查（参考200,000B），在稳定时刻测 `before_drop-after_drop` 的 cache-owned 实际分配；断言实际不超该预算，并用紧差值验证 ledger（不超过1%或256B，按样本选择明确固定口径）。正确实现必须绿，生产 `heap_usable_size` 退成恒等size的 H 变异必须红。参考 probe 只取必要测量/样本，循环有明确上界，不整份复制 REVIEW-ONLY 文件、不开新测量框架。

Reviewer 已测正确helper为 ledger/owned均199,680B；H变异为ledger199,776B、owned218,112B。这是**交付证据缺少判别力**，不是当前生产cache越界；9bed83d归因更正继续有效，原4MiB契约不变，不改生产helper或增加裕量来修不存在的问题。

两个子代理分别负责 Core时钟 与 独立测试（文件隔离），主执行代理整合、验证共享起点回退/H回退各自精确红后恢复绿，再跑默认、标准观测、H、I0及打包验签。只报告本批完成，整体仍 BLOCKED：原因UI、逐语言warm校验/失败处置、焦点/中心阅读优先准入仍待后续，I3与测试清理未执行。该补修落实已有清单的证据要求，不改变已批准产品范围。

## 阶段 B 补修 #2 签字与剩余三项实施清单（待规划复审）

Reviewer `w3:p7` 对 **40cb90d** 回调“本批通过，P1:0”，证据 `/private/tmp/glidemark-phase-i-i12-rework4-stage-b-fix2-review/REVIEW_REPORT.md`。共享start_ms重置变异红；独立malloc_size/drop差分小预算检查的H变异红（218,112B>200,000B），正常ledger/owned均199,680B。默认 **322/0/0、47结果行**，标准观测 **23/0**，H **21/0**（8项与默认重叠），I0颜色6项/hidpi8项在默认；二进制15:02:27晚于提交15:02:10，严格验签通过，无真机像素。

本批并未结束整个B。残余P2：now_ms仍带可单独重置的Instant参数，native-only重置变异不可见；边界样本未assert逼近预算且有无用_residual。下一批分别用最小无参单调时刻接缝（每解析只保留start_ms一个起点）及ledger>150,000B/删无用测量收口，不另开计时或记账框架；已有红灯和硬预算不得放宽。

剩余三项沿用1c9bbd1/16666bf已批契约，以下具体接线提交Reviewer确认后才派实现：

1. **原因真正可见，保持正文几何。** 当前SyntaxBlockState/localized_reason仅写无UI读。复用现有状态和i18n，将实际当前可见降级块的原因读进两侧UI；初始化失败、累计预算耗尽、大小/行数/单行1000标量限制、映射错误、资源不足分别表达，含“原文可读”。未知/无语言及空块正常Plain，不误报失败；取消和等待不伪装终止失败。建议在现有pane相对容器内用一个不占正文布局的状态提示（源码沿用右下行列状态样式、预览同位置），显示当前最优先失败块的源行/原因及其余数量，完整列表用同一提示的tooltip，不为每个代码行加元素或增加行高。提示本身不进入复制文本、不拦文本拖选；滚出、切文档、恢复成功立即消除旧原因。Reviewer需明确该最小呈现能否满足“块的可辨原因”，若不能，指出必要的最小块定位方式。两侧显示与着色共用同一Workspace块状态，不另存一份spans；MappingInvalid不能因未进入队列漏报。
2. **逐语言warm就绪与失败处置。** 语法集初始化、十种代表性多行样本预热及校验全部后台。逐canonical语言用常规bounded路径验证预算内且有预期非Plain角色；仅“SyntaxSet已初始化”不代表该语言ready。首次校验未通过最多额外一轮同语言预热/复验，仍失败返回InitFailed可见原因，不消耗用户块的BudgetExceeded终止次数。用户块等待对应语言ready后解析；内容首超限一次重试、再次失败给预算原因，代次0/1和每代两次总<=4继续成立。进程共享只保留固定十种语言的有限warm状态；不在OnceLock初始化闭包里递归调用会再次初始化该锁的解析入口。失败/重试结果仍通过doc/revision/request守卫；切文档/内容/明确刷新规则保持。不得用draw、保存、模式或淘汰增加warm代次或用户块次数。
3. **实际几何驱动稳定准入。** 复用两侧既有bounds/regions/VisualLine，候选按焦点窗格优先、到其视口中心的距离、文档位置排序去重；同key多处出现取最高优先位置。使用窗格内相对距离，避免窗口坐标或HashSet迭代决定排序。无焦点时保持最近阅读窗格，初始无记录才按当前模式/文档顺序稳定回退；只在真实焦点或可见候选及其排序变化时重评估，普通notify/重复draw不反复排队。现有只比较key集合的提前return需同时识别焦点/顺序变化，不另造通用签名或优先队列。按该固定次序为本需求依次容纳实际Ready结果，能装则保留，不能则标ResourceConstrained并继续检查后续较小结果；同集合不让后完成的低优先结果逐出高优先结果。已有低优先驻留不能阻挡新的高优先候选，不能把所有visible无条件retained后交给insert碰运气。失败候选不保留无预算spans副本；非当前需求LRU可释放，同需求容纳结论稳定，焦点/视口/内容/明确刷新后才重评估。最终两侧同一块当帧同色/同Plain。没有真实editor bounds时先不提交源码需求，等prepaint下一次合并defer，不用800或窗口高度猜测。

**最小证据与交付边界：**

- Core业务与独立测试两子代理文件隔离，主代理负责App/UI整合。优先增强已有目标，不新增独立参考实现；用于制造资源压力的小cache预算只在test feature/测试实例设置，生产4MiB不变。
- 一个真实UI表驱动检查由生产结果触发并实际渲染原因，不能只断言localized_reason字符串或状态map。覆盖五种失败及恢复/滚出/切文档无陈旧提示；删除UI读接线必须红，正文/命中/复制几何与H保持。
- 十语言代表样本正常校验及有限失败/重试用例，观测真实后台入口；去掉warm验证或把失败当ready应红，界定测试钩子只强制底层结果，不在测试里重写状态机。保留正常bash/xml/rust实际颜色，断开生产预热接线能被检查抓住。
- 一条小预算真实Split窗口反例，两侧不同可见块、同key重复、焦点切换及视口中心变化：实际入库/两侧颜色/资源提示与既定顺序一致，重绘不增加请求、滚回可恢复，去焦点/中心排序各自应红。检查insert拒绝和缓存有界；必须能区分只按文档顺序的旧实现。
- 无参时钟共同起点重置、native-only旧起点移除与H边界红灯保留；不复制全历史变异。默认全量/AGENTS原样观测/H/I0/打包严格验签、子代理结束与精确计数后提审。整体仍不能宣称Phase I完成：I1/I2本批还待独立终验，I3与单独测试清理未执行；最终PR授权照旧。

### d5e5186 规划复审回应：六处接线定案（待回审）

Reviewer指出O1—O6，证据 `/private/tmp/glidemark-phase-i-i12-rework4-stage-b-fix3-plan-review/REVIEW_REPORT.md`。认可原设计，以下消除执行端二选一，不新增产品范围：

- **O1，选需求门控，不在解析批次await。** Workspace持固定十语言的 `HashMap<&'static str, WarmState>`，有Waiting/Ready/InitFailed及最多0/1代次；复用一个独立warm任务句柄，一次后台任务完成有限样本预热/验证，不占syntax active/latest槽位。未Ready的可见块为非终止WaitingWarm，不进入用户解析队列、不递增syntax_attempts。语法集合仍进程OnceLock共享，warm就绪表按Workspace持有，不再为共享warm任务造等待者框架。warm完成/最终失败的回调更新该有限表，**显式清空上次已提交的有序需求记录并重新运行当前可见需求合并**，即使可见key完全未变也会推进Ready块或显示InitFailed；不能只notify后被集合相等提前return。任务不捕获旧文档待解析key，不把旧revision结果写回；切文档不取消与内容无关的warm，完成后只重新评估当前文档，用户解析结果仍逐项过doc/revision/request守卫。Workspace销毁则取消回调，后台在样本/语言边界检查取消，单个正则不可抢占的限制不变。warm不阻塞最新用户需求的合并；已Ready语言可先解析，不用等待其他语言warm，不以全局表锁包住样本正则。首次/最多一轮复验仍归warm代次，用户预算次数只在Ready后真实提交解析时记账；每代最多两次/总<=4是上界，不强求失败请求必须用满四次。
- **O2，render期落字段，defer只读快照。** 保留现有 `cx.defer` 的每draw单armed模式。当前源码/预览共用Workspace focus_handle，不能伪称已有两个独立原生焦点；在 `WorkspaceView::render(&mut Window, ...)` 内读取 `focus_handle.is_focused(window)`，只有其真实持焦点时才用现有preview_focused逻辑窗格标记区分Preview/Source并写 `current_reading_pane`。该标记继续由两侧现有实际点击/键盘归焦入口维护，不能在defer里拿陈旧布尔直接冒充焦点；搜索/设置/其他焦点时当前值为None。另存 `last_reading_pane`，仅当前有效时更新，无焦点按它回退；初始无历史才按模式/文档稳定回退。无需新增pane FocusHandle或改变H的Cmd+C/输入路由。render快照随当前需求一起比较，所以只有焦点/优先级变化而key集合不变也会重评估。
- **O3，映射错误只有AST描述一个来源。** Workspace收集可见描述时先看 `ParsedCodeBlock.is_valid`：false的出现位置立即标MappingInvalid，两侧该块Plain+源行原因，不入解析队列、不占attempt。删除无生产者的 `ParseResult::MappingInvalid` 及对应结果匹配臂，保留UI块状态类别。映射错误归属于该revision中的描述源范围/围栏起始行，不把同CodeKey另一处合法出现误报为映射错；源码和预览用同一描述定位该出现位置并在应用spans前门控。缓存身份仍只canonical语言+payload，同内容合法块可以复用结果，不能为此复制payload或另建扫描器。
- **O4，完整准入次序随请求快照传递。** `SyntaxBatchRequest` 除missing解析工作项外，携带该需求的完整有序 `Vec<CodeKey>` 和阅读窗格快照（包含已缓存候选，去重保留最高优先位置），retry沿用同一需求顺序。焦点/滚动/优先级变化会建新request_id并替换latest pending，旧完成仍被现有三维守卫拒绝，不能事后拿新的last_visible顺序解释旧结果。被接受的完成回调仅按**请求自带顺序**评估已有Ready与新结果的容纳：非当前需求条目先按现有LRU释放，本轮已接受较高优先key才进入retained，较低优先条目不能反向阻挡/逐出它；拒绝即资源原因且继续尝试后续小条目。容纳结论用于两侧着色，不以缓存里碰巧还有旧span就越过降级状态。没有missing但排序变化也要立即用该快照重评估现有结果，不等一个不存在的新解析完成。复用Vec顺序/简单retained，不做通用签名、优先队列或第二份spans缓存。
- **O5，接现有完整i18n。** 在 `I18nStrings` 增失败原因/原文可读/数量提示所需的最少字段，覆盖现有全部显式locale表（含繁体），Auto沿用既有resolve；删除 `localized_reason(is_zh)` 中英硬编码分支，状态到strings字段一处映射。HardLimit文案必须含单行1000标量，不只64KiB/2000行。不为此增加翻译依赖或新的语言框架。
- **O6，tooltip不是鼠标事件黑洞。** 提示复用现有状态行样式及 `.tooltip`，窄条且不遮挡行列标签。若其hitbox与正文文本区相交，mousedown/move/up须交回该pane现有生产命中/选区处理，使用原始window坐标，不stop_propagation、不改坐标猜测、不合成另一套事件；仅“不注册点击handler”或“让事件冒泡”不能证明兄弟源码行收到事件。必须用真实事件验证从提示区域起手及拖过提示区域的选取/释放/复制与移除提示时一致，不能只检查某个函数被调用。若GPUI无法复用已有命中入口保证此边界，先报告具体API限制回规划，不静默让tooltip抢拖选。

新增判别边界随上述最小证据并入现有用例：固定可见集WaitingWarm→Ready会实际着色、WaitingWarm→InitFailed会显示且不耗用户attempt；搜索夺焦不误用preview旧标记、归焦变化但key相同会变准入；同key合法/无效两处仅无效处Plain；请求在飞时切焦点使旧结果被拒、最新请求按自己顺序容纳；无missing排序变化仍更新；提示区域真实拖选不被截断。这些是O1—O6的闭环，不能只测状态map或手工构造不合法request状态。原守卫/预算/H变异与计数口径保持。仍待Reviewer对本修订明确无未解决实质异议，未派实现。
