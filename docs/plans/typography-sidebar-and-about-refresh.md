# 排版、侧栏、关于页与更新入口刷新

状态：草稿 v3（已按两轮审查修订，待复审）。用户于 2026-11-06 确认：

1. **配色**：保留暖石纸张背景与产品身份；默认调色板中性化（正文/次要文字、引用块、表格、语法高亮），其他调色板不动。
2. **排版**：预览与编辑器统一；H1/H2/H3 分级间距；默认版心 820→760；编辑器水平内边距 20→32；行号极浅 + “显示行号”开关（默认开）。
3. **设置**：新增独立“快捷键”页；关于页只留产品信息/版本/更新/页脚；版本号灰色；检查更新改次级按钮；右上角在有已下载更新时显示图标+圆点，点击重启更新，移除顶部更新横幅。
4. **本轮**：计划 + 全部实现（分阶段提交），审查通过后执行。

## 0. 首轮审查回应

| 审查项 | 处理 |
| :--- | :--- |
| O1 版心迁移不是一次性 | 新增 `Preferences.config_version: u32`（serde default 0，`Default` 实现为 1）；任何 `config_version < 1` 都置 1，仅当值恰为 820 时改 760，迁移后立即写回；之后用户设 820 保留。补旧 820/旧 750/之后设 820/新默认四组测试 |
| O2 标题 padding 与选区探针脱节 | `heading_spacing(level) -> (pt, pb)` 同时用于元素与 `TextRegionInfo.padding_top`；补 H1/H2/H3 首行/末行真实命中测试，更新 `phase_i` 探针断言 |
| O3 移除 update_ready 导致 About 无文案 | 保留 `update_ready`，仅用于 About 的 Downloaded 状态；图标 tooltip 使用 `restart_update`；仅移除横幅专用的 `later` |
| O4 `.border_1()` 与 settings 源码测试冲突 | About 次级按钮使用 `.border(px(1.))`；现有 `settings_modern_ui_test` 必须保持通过，不放宽/删除 |
| O5 对比度声明矛盾 | `syntax_comment` 改为 `#6E7781`（≈4.5:1）；正文/代码 ≥4.5；`text_muted #8C8C8C` ≈3:1 仅用于辅助信息（用户指定）；行号 opacity 0.55 ≈1.7:1 仅装饰。补 comment/text_muted/行号对比度断言 |
| O6 结构色/旧测试未列 | 引用块/表格颜色抽成 `blockquote_border/table_header_bg/table_border` 纯函数并单测；列出并更新 `clean_palette_and_code_style_test`、`config_test`、`syntax_bullet_and_list_marker_test` 的旧目标值 |
| O7 细节 | 搜索聚焦固定为 1px `.border(px(1.))` + `text_primary.opacity(0.08)`；更新图标新增 `icons/refresh.svg` 并注册；页脚用 `cx.open_url`；update_banner_test 替换为图标测试；验收加 460px 设置窗口滚动截图与暗色语法对比度断言 |
| O1（v2 轮）迁移仍非一次性 | `Preferences::default().config_version = 1`；`load_from_path` 对任何 `config_version < 1` 都置 1（仅值恰为 820 时改 760），并写回；补旧 820、旧 750、之后设 820、新默认四组测试 |
| O2（v2 轮）comment 对比度不足 | `syntax_comment` 改为 `#626C77`（code_bg 上约 4.89:1）；断言背景明确为 `code_bg #F6F5EF` |
| O3（v2 轮）文案不一致 | `update_ready` 仅用于 About 的 Downloaded 状态；图标 tooltip 使用 `restart_update` |

## 1. 事实与目标

| 项 | 现状 | 目标 |
| :--- | :--- | :--- |
| 预览正文 | 字号 16px、行高 1.75 | 保持；正文色 #2C2C2C |
| 标题间距 | 统一 `pt_3/pb_1`（12/4） | H1 32/12、H2 24/8、H3 18/6、H4+ 12/4 |
| 版心 | 默认 820px | 760px（`config_version` 一次性迁移旧默认 820） |
| 编辑器内边距 | 16+4=20px | 28+4=32px |
| 行号 | `text_muted.opacity(0.8)`，间距 6px | `opacity(0.55)`、间距 8px、可关闭（默认开） |
| 引用块 | 4px 橙色、`p_3`、surface 底 | 3px `text_primary.opacity(0.15)`、左 14/右 12、透明底、文字略淡 |
| 表格 | 表头 `theme.bg`、单元格 12px | 表头 `text_primary.opacity(0.02)`、1px `opacity(0.08)`、单元格 12px |
| 侧栏文件行 | 44px、标题 13/日期 11 两行 | 保持（已满足 40–48px 与两行） |
| 搜索框 | 28px、6px 圆角、`theme.bg` | 28px 胶囊 `rounded_full`、`text_primary.opacity(0.04)`、px 8 |
| 关于页 | 快捷键+更新混排、橙色版本、实心橙按钮、英文 Updates | 独立快捷键页；关于页灰色版本/次级按钮/页脚 |
| 更新入口 | 顶部横幅 | 右上角图标+圆点，点击重启；移除横幅 |

## 2. 配色（仅默认调色板）

保留 `bg #F5F4F0`、`surface #FFFFFF`、`border #E6E4DE`、`code_bg #F6F5EF`、`accent #D9532F`（仅用于选择/焦点/主操作）。

浅色默认：
- `text_primary: #2C2C2C`（正文，替代 #1F2328）
- `text_muted: #8C8C8C`（时间/路径/行号等，替代 #787571）
- 语法角色改为低饱和 GitHub Light 近似：
  - keyword `#A43F3F`、attribute `#4A6785`、string `#4F6F52`、number `#8A5A2B`
  - comment `#626C77`（code_bg 上约 4.89:1）、inline_code `#2C2C2C`、link `#4A6FA5`
  - bullet `#8C8C8C`（弱化 Markdown 标记）、code `#3E4A56`
- 深色默认保持暖暗底，仅做同向降饱和：
  - keyword `#E28C8C`、attribute `#9BB7D4`、string `#A8C79A`、number `#E0B080`
  - comment `#8C8C8C`、link `#A8B8D8`、bullet `#8C8C8C`、code `#D8D4CC`

其他调色板（claude/macos/tokyo/nord/github/solarized/one-dark）颜色不动；引用块/表格使用 `text_primary.opacity(...)` 的结构色，因此在所有主题下都保持中性。

对比度：正文/代码/语法注释在各自实际背景（代码注释背景为 `code_bg #F6F5EF`）≥4.5:1；`text_muted #8C8C8C` 在暖纸上约 3:1，仅用于日期/路径/状态等辅助信息（用户指定）；行号 opacity 0.55 约 1.7:1，仅装饰。提交时记录实际比值。

## 3. 预览排版

- 标题：`heading_spacing(level)` 返回 `(pt, pb)`：H1 32/12、H2 24/8、H3 18/6、H4–H6 12/4；元素 `pt/pb` 与 `TextRegionInfo.padding_top` 都使用该值（H4+ 12）。字号保持 1.8/1.45/1.25/1.1 倍。H1/H2 底部分隔线使用 `text_primary.opacity(0.08)`。
- 正文：保持 1.75 行高与 16px；颜色取 `text_primary`（默认 #2C2C2C）。
- 引用块：`border_l` 3px `text_primary.opacity(0.15)`，左内边距 14、右 12、上下 10，透明底，整体 `opacity(0.88)`；不再使用 accent。
- 表格：外层/行/表头分隔线统一 `text_primary.opacity(0.08/0.06)`；表头背景 `text_primary.opacity(0.02)`；表头字重 SEMIBOLD、字号 0.95 倍；单元格保持 12px 内边距。
- 代码块：保持现有 `code_bg`；语法色来自默认调色板的新值。
- 版心：默认 760px；`Preferences` 新增 `config_version`（serde default 0；`Default` 实现为 1）。`load_from_path` 中任何 `config_version < 1` 都置 1；仅当值恰为 820 时改为 760；迁移后立即写回。之后用户主动设 820 保留。

## 4. 编辑器

- `EDITOR_CONTAINER_PADDING_X` 16→28（加行内 4 = 32px）；`EDITOR_GUTTER_MARGIN_RIGHT` 6→8。
- 行号：`text_muted.opacity(0.55)`；新增 `Preferences.show_line_numbers: bool`（serde default true）；关闭时行号列保留宽度但文本为空，避免文字横向跳动（或整列收窄？选择保留宽度，保证光标/换行稳定）。
- 设置 → 编辑与保存：新增“显示行号”开关（默认开），持久化；测试关闭后 `editor_gutter_*` 文本为空且布局不跳。
- 预览与编辑器的横向安全留白一致为 32px；预览版心 760，编辑器保留整窗宽度（源码编辑需要宽度），仅统一内边距。

## 5. 侧栏与搜索

- 文件行：保持 44px 与两行（标题 13px/日期 11px 浅灰）；仅把标题在选中态保持 MEDIUM，未选中 NORMAL（现状）。
- 搜索框：`h(28)`、`rounded_full`、`bg(text_primary.opacity(0.04))`、hover 0.06、水平 padding 8、图标 14、输入 13px；聚焦态固定为 `.border(px(1.))` + `border_color(text_primary.opacity(0.08))`（不新增高饱和色）。
- 搜索框宽度随抽屉，保持现有搜索/清空/进度行为不变。

## 6. 设置：快捷键页与关于页

### 6.1 新“快捷键”页

- `SettingsTab` 增加 `Shortcuts`，导航位于“界面语言”与“关于”之间；图标新增 `icons/keyboard.svg` 并注册到 `AppAssets`。
- 内容：沿用现有六条快捷键（打开/保存/导出/目录/源码·分屏·预览/设置），使用与其它页一致的分节标题（11–12px、SEMIBOLD、text_muted、下方 6–8px）。
- 关于页移除快捷键列表。

### 6.2 关于页

- 顶部：72px logo + 名称/简介/版本；版本号使用 `text_muted`（不再 accent）。
- “软件更新”分节：小标题 11–12px SEMIBOLD muted；状态文本（检查中/已是最新/失败/已下载）用 muted；按钮改为次级样式：
  - 高 26px、左右 padding 12、圆角 6、`bg(text_primary.opacity(0.05))`、`.border(px(1.))` + `border_color(text_primary.opacity(0.12))`、文字 `text_primary`、12px；不得使用 `.border_1()`（现有 settings 源码测试要求零 `.border_1()`）；
  - hover `opacity(0.08)`；仅当 `UpdateStatus::Downloaded` 时在按钮旁显示一个 6px accent 圆点（不把整颗按钮染成主色）。
- 页脚：11px muted “Made with Rust & GPUI · Open Source on GitHub”，点击打开 `https://github.com/2716190547/zmark`；与内容底部留 16px。
- 语言统一：移除硬编码英文 “Updates”，使用 i18n `software_updates`。
- 版式与其他设置页一致：分节标题 + 内容、`gap_5`、左对齐；窗口高度不足时沿用现有滚动。

## 7. 更新入口

- 移除顶部更新横幅及其 `update_banner_*` 状态、`later` 文案与相关测试。
- 主窗口右上角动作区（导出/设置左侧）在 `UpdateStatus::Downloaded(version)` 时渲染更新图标：
  - 新增 `icons/refresh.svg` 并注册 `AppAssets`（不复用导出按钮的 download 图标）；右上角 6px accent 圆点 badge；
  - tooltip：`{restart_update} v{version}`；
  - 点击调用 `restart_for_update(Some(window), cx)`，复用未保存文档关闭流程；
  - 复用按压动效与状态化 id（`press_btn_update_ready_{generation}`）。
- 无已下载更新时不显示图标；About 的“检查更新”仍可手动触发。
- 导出成功横幅保留（与更新无关）。

## 8. i18n 与偏好

- 新增字段（10 locale）：
  - `show_line_numbers_label` / `show_line_numbers_desc`
  - `software_updates`
  - `footer_credit`
- 保留 `update_ready`，仅用于 About 的已下载状态；图标 tooltip 使用 `restart_update`；移除横幅专用的 `later`。
- `Preferences` 新增 `show_line_numbers: bool`（default true）；`max_column_width` 默认 760 + 820→760 迁移。

## 9. 测试与验收

- 配置：`show_line_numbers` 默认/往返；`max_column_width` 820→760 迁移，其他值保留。
- 主题：默认浅色 `text_primary == #2C2C2C`、`text_muted == #8C8C8C`；语法 bullet/comment 为中性；对比度脚本/断言记录。
- 预览：`heading_spacing(level)` 纯函数测试（32/12、24/8、18/6、12/4）；引用块/表格结构色由纯函数断言，不使用“可选”验收。
- 编辑器：关闭行号后 gutter 文本为空、布局宽度不变；行号颜色/间距由现有几何测试覆盖或新增。
- 搜索：`drawer_search_container` 高度 28、圆角/背景（debug bounds + 源码断言二选一，优先几何）。
- 设置：导航含 5 项；快捷键页渲染；关于页无快捷键、版本为 muted、按钮为次级样式；更新图标在 Downloaded 时出现、Idle 时消失。
 - 既有测试更新（目标值变更，不放宽）：`clean_palette_and_code_style_test`（text_primary/syntax 新值）、`config_test`（默认 760/config_version）、`syntax_bullet_and_list_marker_test` 的 palette matrix；`settings_modern_ui_test` 保持零 `.border_1()` 通过。
- 对比度：断言 `text_primary`/`syntax_comment` ≥4.5、`text_muted` ≈3、行号 ≈1.7（记录实际值）；暗色默认同样断言 comment/keyword ≥4.5。
- 引用块/表格：`blockquote_border/table_header_bg/table_border` 纯函数断言使用 `text_primary.opacity(...)`；不做“可选”验收。
- 命中：H1/H2/H3 首行/末行点击 anchor 正确（使用 `heading_spacing` 的 pt）。
- 更新入口：`update_banner_test` 替换为 `update_ready_icon_test`（Downloaded 出现、Idle 消失）；不保留横幅测试。
- 全量：`cargo test --workspace`、观测专项、Phase H 串行；`./scripts/bundle_macos.sh`；截图对照（分屏/纯预览/设置-关于/设置-快捷键/搜索框/行号关闭/460px 高设置窗口滚动）。
- 人工：暖纸底不变；语法标记淡入背景；引用块无橙色；表格轻量；关于页风格与其它页一致；右上角图标点击走保存提示后重启。

## 10. 风险与取舍

- 默认色板中性化会削弱暖纸+陶土橙的强品牌感；accent 仅保留给选择/焦点/主操作。
- 820→760 迁移会覆盖“恰好使用 820”的用户自定义值；仅迁移精确旧默认值，其他值不动。
- 移除更新横幅后，用户可能错过更新；右上角图标 + 圆点 + 关于页状态提供两处入口。
- 行号关闭时保留列宽，可能让源码左侧留白略大；换取布局稳定。
- 设置页新增 tab 与关于页重构需确保小窗口可滚动；沿用现有滚动容器。
- 深色调色板的语法降饱和需要单独目检，避免低对比。

## 11. 独立审查记录

- 首轮（v1）：结论“有异议”，O1-O7 已写入 §0 并修订。
- 第二轮复审（v2）：仍“有异议”；O1-O3（config_version 覆盖所有旧版本、comment 对比度、update_ready 文案）。v3 已逐条处理并写入 §0。
- 第三轮复审（v3）：**结论“对该 revision 无未解决实质异议”**。审查版本 SHA-256 `338fb4e7c6fc3027924b185f90261dfe20d2ca2e77e7e09567edba2f7550d567`。实施按“config/theme → 预览排版 → 编辑器/搜索 → 设置/关于/更新入口”分阶段提交；迁移写回失败只记录日志，不阻断启动。
