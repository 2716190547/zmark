# Drawer 搜索、文件夹与文件行 UI 优化及小批测试清理

版本：v1（用户于 2026-11-05 明确取消既有审查流程，授权直接开发；记为“用户覆盖”，不写成审查员无异议）  
基线：`9991a31556a97918302486722268c2d35691d073`  
用户决策：2026-11-05 已确认 macOS 克制紧凑方向、调整文件行，并同意有证据地删除 9 项低价值测试。

## 目标与边界

针对用户截图中搜索框输入后拥挤、文件夹层级不清的问题，只调整 Drawer 呈现与已确认的低价值测试，不改变搜索、中文 IME、折叠、新建、选择、关闭或重名消歧行为。

生产代码只改 `crates/glidemark_app/src/views/drawer.rs`。复用现有 `ThemeColors` 与 `icons/chevron-right.svg`；不新增依赖、资产、状态字段、通用组件、测试框架或 ADR。`note — ZGWS` 的 Workspace Root 重名消歧规则保持不变。

## UI 方案

### 搜索槽

- 保持现有 28 逻辑像素高度；截图为 2×，不按物理截图像素误判控件高度。
- 容器左右内边距由 8px 增至 10px，搜索图标与输入文字间距由 6px 增至 8px；输入字号固定 13px，清除按钮保留足够命中区域。
- 用户后续确认：搜索框底色与 Drawer 的 `theme.bg` 完全一致，去掉静态边框和独立色块；仅悬停时复用 `theme.hover_bg()`，现有光标继续提供聚焦反馈，不添加阴影或新聚焦状态。
- 光标宽度收窄并缩短上下两端；只改绘制几何，不改 `EntityInputHandler`、选区、IME 或键盘处理。
- 占位文字由硬编码黑色透明度改为继承文字色的透明度，避免暗色 Palette 对比度错误；输入正文保持 `text_primary`，辅助图标保持 `text_muted`。
- 清除按钮悬停色改用主题前景色透明度，不再使用在亮色主题中近乎不可见的白色透明层。

### 文件夹与 Workspace Root

- 根目录和子目录均增加 10px 左右的 `chevron-right.svg`，展开时通过 GPUI `Transformation::rotate` 旋转 90°；不改原有整行点击折叠行为。
- 保留 open/closed 文件夹 SVG。根目录使用 13px Semibold，子目录使用 13px Medium；图标使用 `text_muted`，名称使用 `text_primary`，悬停沿用 `hover_bg()`。
- 保留现有行高、右键移除根保护、hover 新建文件按钮和路径 tooltip；不添加数量徽章、彩色文件夹或额外卡片。

### 文件行

- 保留 44px 双行结构、13px 文件名与 11px 日期；日期仍来自缓存的 `node.mtime`。
- 调整文件行左侧缩进，使文件名与文件夹层级有明确对齐关系，日期与文件名共用同一文本起点。
- 选中态继续复用 `selection_bg()` / `selection_text()`，通过间距、圆角和文字层级减轻当前大块选中区域的笨重感；不引入新的强调色或状态。

## 小批测试清理（独立测试提交）

只删除下列已逐项确认的低价值检查，共 9 项；不按年龄、数量或目标比例删测，不使用 `ignore`、放宽断言或缩小输入。

1. 删除整个 `crates/glidemark_app/tests/drawer_polish_test.rs` 的 5 项：全部仅用 `include_str!` 和源码字符串匹配约束旧样式、旧行高或禁止旧色值，没有经过生产 UI 行为入口，且直接阻碍已批准的视觉调整。
2. 从 `sidebar_two_tier_and_header_weight_test.rs` 删除 2 项：
   - `test_sidebar_two_tier_typography_and_arrow_removal`：锁定旧字重与字符箭头实现；本次使用 SVG 箭头，旧视觉契约不再成立。
   - `test_sidebar_two_line_file_items_and_single_line_folders`：与已删除的 Drawer 样式字符串检查重复，只锁定 44/28px 和字号字符串。
3. 从 `drawer_search_test.rs` 删除 2 项：
   - `test_drawer_render_contract`：只检查源码 token、字符串顺序和 emoji 是否存在，不能证明搜索框固定、输入、清除或状态提示的实际行为。
   - `test_drawer_search_params_from_state`：测试自行构造 `DrawerSearchParams`，再断言自己写入的字段；没有生产接线或行为判别力。

明确保留：中文 IME、光标/选区、搜索过滤与恢复、后台搜索和陈旧结果、文件夹展开状态、文件创建/关闭、资产真实加载，以及有有效算法或生产入口价值的其他检查。本批不处理全仓其他候选，不声称 9 项删除会显著提速；减少一个集成测试二进制及脆弱维护成本是已确认收益，实际耗时如实测量。

## 实施与验证节奏

1. UI 单独提交：只改 `drawer.rs`。
2. 开发中只运行：
   - `cargo test -p glidemark_app --test drawer_search_test --test sidebar_two_tier_and_header_weight_test --test asset_registration_test`
   - 必要时按名称运行 IME/选区检查，不在每次样式微调后运行全量 300+ 测试。
3. 启动真实应用检查默认暖石亮色：空搜索、中文输入、长查询、清除、文件夹展开/收起、文件选中与日期对齐；再抽查一个暗色 Palette。保存实际截图证据，不用自动像素测试替代人工检查。
4. 测试清理单独提交，逐项对应上述失效证据；提交前跑上述相关目标。
5. 最终候选按项目既定交付门槛运行 `cargo test --workspace`、AGENTS.md 要求的 feature 专项及 Phase H 21 项串行回归，分报 passed/failed/ignored 和重叠；随后运行 `./scripts/bundle_macos.sh`，检查严格签名与二进制时间。有限的中途验证不冒称完整回归。

## 验收

- 输入中文后，搜索图标、正文、光标和清除按钮之间有清晰、稳定的水平与垂直留白，无裁切或 IME 回归。
- 文件夹展开状态一眼可辨，根/子目录层级明确；文件行和日期对齐，选中态仍清晰但不过重。
- 搜索、折叠、新建、选择、关闭、右键移除根及重名消歧行为不变。
- 只删除批准的 9 项低价值测试；功能保护不减少，最终交付门槛不被削弱。

## 独立规划审查

用户于 2026-11-05 明确指示“以前的模式不用了，直接开发即可”，覆盖原审查关口；本计划不宣称已取得独立审查签字。
