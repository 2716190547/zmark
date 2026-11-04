# I1/I2 返工 #4：分阶段补齐已批准契约

现行设计仍是独立签字 **1c9bbd1**，详见 [主规划](phase-i-color-and-media.md) 末节及 R1–R5。本清单只缩小每次交付/审查的范围，不增加新产品选择或重新解释资源预算。阶段 A 独立审过再派 B；不把部分修复称为 I1/I2 整体完成。

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

1. 在生产 parser 的事件/offset 路径产出同 revision 的代码描述和逐行映射：canonical payload/语言、payload 行范围、source content/前缀范围、EOL 宽度、synthetic LF。适用于顶层及 ListItem.body/BlockQuote 内代码；不在 App 再逐行扫 fence 拼文本。源码只消费描述，映射先校验非 EOL 字节相等及 UTF-8 边界；不等块两侧 Plain 且有可辨错误，不改源文本。
2. 删除 `extract_source_code_blocks` 及其生产调用。源码可见着色、代码几何范围、预览需求取同一描述的内容 key；同一块不再因 CRLF/容器前缀产生两个条目。原文件/保存/源码复制保持，预览代码复制保持 canonical 语义。
3. 删除进程 static CACHE 的生产读写，包括批次双写、preview.or_else(global)、旧 helper 的生产旁路。测试改为实例缓存或纯 parse，不为旧用例保留可被生产调用的隐式全局副本。仅 SyntaxSet/受限预热可进程共享。切文档清空本 Workspace 不应再从其他缓存回填旧角色。
4. 内容 key 用共享 payload/固定 canonical lang，避免 map/deque/retained/region 每次复制整段 String。缓存、索引、容纳集合中实际保留的 key/角色数据和容器 capacity 均须在 4MiB 额度内记账；同一 Arc allocation 只计一次。不能继续 `len + 64` 冒充实际容量。优先把重复容器压成已有小型具体结构，避免新框架；若 HashMap 不便给可靠容量界，允许采用更简单可核算的单一容器。不把帧临时分配和持久保留混成一项，也不将文档/纯几何数据算作已受 4MiB 限制。
5. 入库返回明确准入/拒绝结果供 B 消费，不能在超额时对调用者静默成功；本阶段维持预算上限，不增加条数上限。预算/取消语义和重试的完整行为仍在 B 验收。

### A 的最小独立证据

- 一个经真实 `parse_markdown` 的 12 组映射表，沿已核准 probe 的输入：LF、CRLF、列表空格/Tab/CRLF、引用及引用内列表、无尾 LF、缩进、空/空白/CRLF 空行。验证源切片/前缀/EOL/synthetic LF 与 canonical 投影；普通 Rust 小块的 CRLF/列表/引用另经真实 Workspace draw 验证源码和预览最终颜色，同 key 一次解析，不只检查手造 descriptor。
- 实例隔离/切文档检查证明不存在 global fallback；去掉 Workspace 中条目不能从旧缓存恢复。测试不得自己预热被观测的 Workspace 目标来假装元素接通。
- 小型 cache-only 分配/容量检查验证保留内存与 ledger，含 key 共享、容器扩容、淘汰和拒绝；独立测量区分持久/临时，不把两份 Arc 引用误算成两份 payload。核对额度边界附近仍不超限，不用错误统计自证。
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
