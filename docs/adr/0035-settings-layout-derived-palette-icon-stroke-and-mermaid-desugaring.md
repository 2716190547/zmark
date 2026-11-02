# ADR 0035: 无缝贯穿设置视窗、系统化调色推导、图标线宽归一、点部语义上色与 Mermaid 语法脱糖

## 状态
已通过 (Accepted) - 由审查端复审通过，进入分步实施阶段 (Ticket F1 ~ F5)

## 上下文 (Context)
在 Phase E（软折行与 CJK 点击对齐）交付并全绿验收后，用户提供了 3 张核心视觉与功能标注图（图 1 设置视窗问题标注、图 2 标杆妙言排版与色彩样张、图 3 主视窗问题标注）：
1. **设置窗口割裂上下堆叠 (图 1)**：当前设置视窗在顶层硬编码了一个 44px 的顶部横条，包含了红绿灯占位与“设置”文字，将窗口横切成上下两截，下方才是左右两栏。与主 App 的抽屉贯穿风格以及 macOS 原生设置界面不一致，割裂感严重。
2. **外观模式等控件选中色生硬倒错 (图 1)**：当前分段控件（跟随系统/浅色/深色）在浅色模式下，选中态应用了灰蒙蒙的 `warm_pill_bg`（`#EFF1F4`），而未选中项反而是纯白的 `surface` 卡片，产生“未选中像被选中浮起、选中反像被按压禁用”的视觉倒错，且未结合主题色系进行自然推导。
3. **顶栏设置图标常亮错误 (图 3)**：顶栏右侧设置按钮判断依据为 `self.settings_window.is_some()`，一旦打开过设置窗口便永久常驻高亮，即便窗口已关闭仍处于激活态，误导用户为双态开关。
4. **顶栏图标线条粗细不均 (图 3)**：当前 16px 工具图标在不同几何形态下视觉黑度差异巨大：密集复杂图标（齿轮设置、软盘保存、展开书籍）视觉面积过大、糊成一团，而线条稀疏图标（代码、下载）相对显得单薄，线条粗细感觉极不协调。
5. **Markdown 点部与列表标记缺乏色彩节奏 (图 3)**：对比图 2（妙言），当前编辑器列表符号（`- `、`* `、`1. `）、任务列表复选框、引用线 `>`、代码块围栏 ` ``` ` 仅为纯单色黑白或普通纯文本，缺乏妙言式的雅致淡彩色呼吸感。
6. **Mermaid 复合连线与类修饰符报错崩溃 (图 3)**：用户在图表中输入复合连线（如 `A & B --> C` 或 `& N2_2 & N2_3 -> N3`）及类修饰符（如 `:::code`）时，`rusty-mermaid` 不支持此类高级简写语法，抛出 `ParseError { kind: UnexpectedToken, span: ... }` 并回退为大块红框错误。

---

## 决策 (Decisions)

### 1. 设置视窗无缝贯穿左右分栏重构 (Ticket F1)
- **彻底移除 44px 顶部横向标题条**：
  - 彻底去除 `h(px(44.)).border_b_1()` 顶栏容器；
  - 窗口根级直接采用全高横向 flex 双栏容器：左侧为 160px 导航侧栏，右侧为全高设置内容滚动区。
- **左侧导航栏 Edge-to-Edge 贯穿与红绿灯内嵌**：
  - 窗口左上角预留 36px 顶部安全边距（`pt(px(36.))`），供 macOS 原生红绿灯直接内嵌展示；
  - 搜索框与各个分类导航项从红绿灯下方平滑排布，左侧边框与底色纵贯到底，完全对齐 macOS 现代设置与主 App 侧栏风格。

### 2. 分段控件 macOS 凹槽浮雕质感与基于 Palette 的自适应推导机制 (Ticket F2)
- **杜绝硬编码颜色，建立 ThemeColors 衍生推导 Token 族**：
  在 `ThemeColors` 中新增 5 个自适应推导函数，消除任何硬编码 `#FFFFFF` 或 `#343942`，使所有分段控件随当前调色板自适应呼吸：
  1. `segmented_track_bg(&self) -> Hsla`（凹槽底色）：
     - 浅色模式：`self.text_primary.opacity(0.06)`（以当前主字色为基底微透明化，在浅色底上呈现出天然柔和且与主题字色同相的微灰凹槽）；
     - 深色模式：`self.surface.lighten(0.03)`（基于当前表面色微弱提亮，形成下沉式浅凹槽）。
  2. `segmented_thumb_bg(&self) -> Hsla`（激活卡片浮雕底色）：
     - 浅色模式：返回 `self.surface`（纯白卡片，与凹槽底形成鲜明浮雕对比）；
     - 深色模式：返回 `self.surface.lighten(0.08)`（比背景与凹槽更明亮的实体浮雕层）。
  3. `segmented_thumb_border(&self) -> Hsla`（激活卡片微边框）：
     - 浅色模式：`self.border.opacity(0.45)`（1px 极微弱发际轮廓，烘托浮雕实体感）；
     - 深色模式：`self.border.opacity(0.65)`。
  4. `segmented_thumb_text(&self) -> Hsla`（激活卡片主字色）：
     - 返回 `self.text_primary`（高对比主文本色），字重设定为 `FontWeight::MEDIUM`。
  5. `segmented_inactive_text(&self) -> Hsla`（未激活选项字色）：
     - 返回 `self.text_muted`；悬停（Hover）时平滑提升至 `self.text_primary`。
- **Segmented Control 结构规范**：
  - 容器槽底：`bg(theme.segmented_track_bg()).rounded_lg().p(px(2.))`；
  - 未选中态项：背景完全透明（`transparent_black()`），无任何边框，字色为 `theme.segmented_inactive_text()`；
  - 选中态项：`bg(theme.segmented_thumb_bg()).border_1().border_color(theme.segmented_thumb_border()).rounded_md()`，字色为 `theme.segmented_thumb_text()`；
  - 彻底纠正“选中发灰、未选中反白”的倒错感，全面覆盖外观模式切换、自动保存开关、Tab 宽度等所有设置卡片。

### 3. 顶栏设置状态修复与 16px 视口光学重量平衡矩阵 (Ticket F3)
- **设置按钮恢复标准 Action 行为**：
  - 将 `btn_settings` 的 `active` 状态从 `self.settings_window.is_some()` 强制修正为 `false`，取消错误常亮，恢复为仅在 Hover 与 MouseDown 触发反馈的标准操作按钮。
- **渲染像素与 SVG viewBox 单位换算铁律**：
  - SVG 图标原生声明 `viewBox="0 0 24 24"`；在 GPUI 中通过 `.size(px(16.))` 渲染时，布局缩放因子为：
    $$S = \frac{16.0}{24.0} = \frac{2}{3} \approx 0.6667$$
  - 实际渲染线宽计算公式：
    $$\text{Rendered Stroke (px)} = \text{viewBox Stroke Width} \times \frac{2}{3}$$
  - **严禁直接把 viewBox stroke-width 改成 1.5**：若改为 1.5，渲染后实际仅有 $1.0\text{px}$，不仅未能协调，反导致线条过细虚化！
- **光学黑度感知平衡矩阵 (Optical Weight Balancing Matrix)**：
  根据图标图元的几何疏密分布，精确配置各图标的 SVG viewBox `stroke-width`，确保最终在 16px 视口下的视觉光学重量（Perceived Ink Density）绝对均一：

| 图标名称 | 几何特征 | viewBox `stroke-width` | 16px 渲染实际线宽 | 调整目标与光学平衡原因 |
| :--- | :--- | :--- | :--- | :--- |
| `code.svg` (`< >`) | 稀疏开放角线，中间大面积镂空 | **2.25** | **1.50px** | 保持原有线宽，避免线条过细单薄 |
| `columns-2.svg` (`[ \| ]`) | 规则矩形框 + 单中轴竖线 | **2.25** | **1.50px** | 保持标称线宽，与 code 视觉平衡 |
| `download.svg` (箭头+托盘) | 单竖线箭头 + 开口托盘 | **2.25** | **1.50px** | 保持标称线宽，通透有力 |
| `book-open.svg` (翻开书) | 双书页折线 + 弧形曲线交织 | **2.00** | **1.33px** | 微收线宽，避免曲线交界处墨水堆积 |
| `save.svg` (软盘) | 软盘轮廓 + 金属窗 + 门槽多重套叠 | **2.00** | **1.33px** | 微收线宽，释放内嵌门槽内部空间 |
| `settings.svg` (齿轮) | 6 复杂外齿牙 + 内嵌同心圆环 | **1.80** | **1.20px** | 消除内外圈狭窄缝隙糊成黑坨，使光学黑度与 code (1.50px) 达到等重平衡 |

### 4. 8 套主题明暗完整矩阵之点部语义色 (`syntax_bullet`) (Ticket F4)
- **Token 矩阵新增 `syntax_bullet` 规范**：
  在 `ThemeColors` 中正式新增 `syntax_bullet: Hsla` 字段，专门用于列表项标记（无序圆点 `-`/`*`、有序序号 `1.`、任务复选框 `[ ]`/`[x]`）以及语法标点符号的跳跃点缀。
- **8 套调色板 16 色明暗全量矩阵表**：

| 调色板 ID | 主题定位 | 模式 | 点部点缀色 (`syntax_bullet`) | 色彩语义说明 |
| :--- | :--- | :--- | :--- | :--- |
| **default** (妙言中性) | 纯净极简标杆 | 浅色 | `#0969da` (妙言经典青蓝) | 标杆冷青色点缀，清亮跳跃 |
| | | 暗色 | `#58a6ff` (冰川天蓝) | 暗场柔和高亮，通透不刺眼 |
| **claude** (Claude 暖色) | 温润阅读质感 | 浅色 | `#d97706` (温暖琥珀金) | 契合暖棕石灰底色，温润雅致 |
| | | 暗色 | `#fbbf24` (暖金晨曦) | 暖色调提神微光，烘托氛围 |
| **macos** (macOS 活力) | Apple 原生系统 | 浅色 | `#007aff` (系统活力蓝) | 纯正 Apple HIG 强调色点缀 |
| | | 暗色 | `#0a84ff` (系统霓虹蓝) | 暗色模式官方高可读性蓝 |
| **tokyo-night** (东京夜) | 赛博暗夜霓虹 | 浅色 | `#2e7de9` (深邃晴空蓝) | 配合靛蓝/紫罗兰的晴空标缀 |
| | | 暗色 | `#7aa2f7` (东京夜空蓝) | 标志性荧光青蓝点缀，节奏感鲜明 |
| **nord** (北欧极光) | 极地冷色极简 | 浅色 | `#5e81ac` (极地深蓝) | 冰雪苔原中的沉稳冷蓝点 |
| | | 暗色 | `#88c0d0` (极光冰蓝) | 极光冷青点缀，通透空灵 |
| **github** (GitHub 官方) | 工程师硬核标称 | 浅色 | `#0969da` (GitHub 链接蓝) | 官方标称列表与语法点缀 |
| | | 暗色 | `#58a6ff` (GitHub 暗天蓝) | 官方暗场标称语法高亮蓝 |
| **solarized** (日光温和) | 护眼经典双底色 | 浅色 | `#268bd2` (日光深蓝) | Ethan Schoonover 原生经典蓝 |
| | | 暗色 | `#2aa198` (日光青碧) | 原生 Cyan 青碧点，柔和护眼 |
| **one-dark** (One Dark) | 现代代码工坊 | 浅色 | `#4078f2` (One 纯粹蓝) | Atom 标称纯净蓝 |
| | | 暗色 | `#61afef` (One 明亮天蓝) | 现代暗场代表性清爽标缀 |

- **Tokenizer 语法识别与渲染**：
  - 列表前缀识别：无序列表前缀（`- `, `* `, `+ `）、有序列表前缀（`1. `, `2. `）、任务列表标记（`- [ ]`, `- [x]`）；
  - 标记赋予专属 `theme.syntax_bullet`，正文内容保持纯正中性深碳灰，实现妙言特有的视觉节奏；
  - 预览区无序列表圆点（bullet marker）与有序序号统一使用 `theme.syntax_bullet` 渲染。

### 5. Mermaid 词法状态机、边界保护与复合连线笛卡尔积脱糖规范 (Ticket F5)
针对 `rusty-mermaid` 不支持 `&` 复合连线与类名修饰符导致的 `UnexpectedToken` 报错红框，在 `glidemark_core::mermaid::sanitize_mermaid` 中引入严格的词法扫描脱糖预处理器，绝不采用 naive split：

#### 1. 词法保护与直通白名单 (Bypass List)
- 以下语句类型直接跳过脱糖，保证绝对不被修改：
  - 注释行：`%% ...`
  - `classDef` 样式定义行：以 `classDef ` 开头（例如 `classDef code fill:#f9f...`，必须 100% 保留）
  - `class` 赋值行：以 `class ` 开头
  - `style` / `linkStyle` / `subgraph` / `end` / `direction` 行

#### 2. 引号与形状标签掩码保护 (Quote & Label Masking)
- 节点标签内部可能包含任何特殊字符，例如：
  `N3["3. merge_recall (&)"]`、`A["用户 & 密码"]`、`B("A & B 运算")`。
- **扫描状态机**：
  - 维护 `in_quotes` 布尔状态（遇到未转义 `"` 翻转）；
  - 维护 `bracket_depth` 深度计数器（遇到 `[`、`(`、`{` 递增，遇到 `]`、`)`、`}` 递减）；
  - 维护 `edge_label_depth`（在边语法内部遇到管道符 `|label|` 时进入保护态）；
- **保护铁律**：**所有处于引号内、形状标签内或边管道标签内的 `&` 字符一律被视作字面量，绝对不参与节点分割！只有处于保护区外部（裸节点 ID 区域）的 `&` 才是连线复合连接符！**

#### 3. 跨行续行拼合预处理 (Line Continuation Stitching)
- 用户报错实例：
  `snippet: "& N2_2 & N2_3 -> N3[\"3. merge_recall (\"} }"`
- 若一行首个非空白字符为 `&`，或者上一行结尾为指向箭头（`-->`, `---`），说明上一语句跨行未完。
- 在进入分词展开前，自动将续行无缝拼回至上一行末尾，彻底消除悬空 Leading `&`。

#### 4. 复合连线笛卡尔积展开算法 (Cartesian Product Expansion)
- 提取语句中的连线操作符（如 `-->`, `---`, `-.->`, `==>`, `-->|label|` 等）：
  - 将语句沿操作符安全划分为左侧前驱组（LHS）与右侧后继组（RHS）；
  - 在引号保护区外，将 LHS 按 `&` 拆解为节点集：$\text{Sources} = [S_1, S_2, \dots, S_m]$；
  - 在引号保护区外，将 RHS 按 `&` 拆解为节点集：$\text{Targets} = [T_1, T_2, \dots, T_n]$；
  - 展开为 $m \times n$ 条原子单连线：
    $$\forall S_i \in \text{Sources}, \forall T_j \in \text{Targets} \implies S_i \text{ [Edge]} T_j$$
  - **示例 1（多对一）**：`A & B --> C` $\implies$ `A --> C` 与 `B --> C`
  - **示例 2（一对多）**：`A --> B & C` $\implies$ `A --> B` 与 `A --> C`
  - **示例 3（多对多带标签）**：`A & B -->|ok| C & D` $\implies$ 展开为 4 条原子连线：
    `A -->|ok| C`、`A -->|ok| D`、`B -->|ok| C`、`B -->|ok| D`

#### 5. 类修饰符 `:::className` 安全脱敏 (Class Annotation Stripping)
- 形式：在节点闭合符（`]`、`)`、`}`）之后或裸 ID 之后紧跟的 `:::[a-zA-Z0-9_-]+`；
- 规则：在引号保护区外部，安全剔除 `:::className` 修饰符（替换为单个空格），同时严格保护标签字符串内部的双冒号（如 `["std::vector"]`）不受任何影响。

---

## 阶段规划与实施工单 (Phase F Tickets)

| 工单 ID | 标题 | 核心改动模块 | 交付标准 |
| :--- | :--- | :--- | :--- |
| **Ticket F1** | 设置视窗全高贯穿分栏与红绿灯对齐 | `views/settings.rs` | 彻底移除 44px 横栏，左栏纵贯到底，红绿灯安全内嵌，布局测试锁定 |
| **Ticket F2** | 分段控件 macOS 凹槽浮雕质感与 Palette 自适应推导 | `theme.rs`, `views/settings.rs` | 消除硬编码灰白倒错，实现 5 项自适应衍生 Token，全调色板色彩完全相衬 |
| **Ticket F3** | 顶栏设置状态修复与 16px 图标光学平衡矩阵 | `views/workspace.rs`, `assets/icons/*.svg` | 设置图标不再常亮，顶栏图标按光学矩阵校准（1.8~2.25 viewBox），黑度均匀，单测锁定 |
| **Ticket F4** | 8 套主题 `syntax_bullet` 矩阵与 Markdown 点部语义着色 | `theme.rs`, `views/editor.rs`, `views/preview.rs` | 8 套调色板 16 色明暗矩阵全量落地，列表圆点/序号/复选框获得专属点缀色，预览区对齐 |
| **Ticket F5** | Mermaid 词法边界保护与复合连线笛卡尔积脱糖 | `crates/glidemark_core/src/mermaid.rs` | 状态机保护引号与标签，复合连线笛卡尔展开，跨行拼合与类名安全脱敏，红框报错归零 |

---

## 验证策略
1. **自动化测试**：
   - 为设置视窗左右分栏、Segmented Control 自适应衍生 Token、设置按钮 active 状态、8 套主题 16 色 bullet 矩阵、带引号/标签/跨行/多对多 Mermaid 笛卡尔脱糖编写独立高覆盖率单测；
   - 保证全量 `cargo test --workspace` 绿灯且用例数稳步正向递增。
2. **视觉与构建验证**：
   - `./scripts/bundle_macos.sh` 打包生成最新 app；
   - 启动测试，验证设置窗口无横条直通分栏、分段按钮槽底浮雕质感、顶栏设置按钮正常熄灭、顶栏图标视觉线宽完全一致、Mermaid 复合图表完美渲染无红框。
