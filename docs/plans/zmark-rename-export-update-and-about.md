# ZMark 改名、HTML 导出交互、内置自动更新与关于页

> 当前代理约定（2026-11-16）：herdr-triad 已弃用；由当前代理自行审查，不向其他 pane 派任务。本文跨窗格与独立代理审查要求仅作历史记录，现行规则见 [代理工作约定](../agent-workflow.md)。

状态：草稿 v8（已按七轮独立审查逐条修订，待复审）。用户于 2026-11-06 确认：

1. **改名**：全面改名到 ZMark（crate/目录/安装包/配置目录/活跃文档/GitHub 仓库）；历史 ADR 与已完成计划不改写。
2. **HTML 导出**：保留，并补上原生保存面板与保存后反馈（当前对草稿文档点击无反应）。
3. **自动更新**：独立公开 releases 仓库 + 自研轻量更新器；自动下载 + 重启提示；单 stable 通道；Ed25519 签名；本轮创建公开仓库并发布 v0.2.0。
4. **关于页**：现有 Z 形 AppIcon 小图标（72px）+ 名称/简介/版本 + 检查更新。

## 0. 审查回应（v1 → v8）

| 审查项 | 处理 |
| :--- | :--- |
| B1 重启更新误用 `cx.quit()` | 复用现有脏文档关闭流程；干净时或脏文档全部处理完后，启动显式 restart helper（等待 PID 后 `open <实际目标 bundle>`）再 `cx.quit()`；取消关闭提示则清 `pending_restart`，此时尚未换位/启动 helper |
| B2 原地安装非原子 | staging 与目标同卷（目标父目录下的隐藏目录），全部校验通过后，在重启时执行“两步可恢复换位”：轮换/清理 `.old.<version>`，`mv 目标 目标.old.<version>`，`mv staging/ZMark.app 目标`；失败反向恢复；helper 在目标缺失时先恢复 `.old.*`；不宣称严格原子 |
| B3 发布包可能未签名 | `bundle_macos.sh` 先在 `target/bundle/ZMark.app` 完成 Ad-Hoc 签名与 `codesign --verify`，再复制到 `/Applications`；release.sh 只压缩已签名的 staging bundle，解压后再次验签 |
| 1.1 改名清单不完整 | 增加完整清单 + `rg` 机械门禁 + 例外清单（见 §1） |
| 1.2 “人类可读文本全改”过宽 | 只改产品身份、用户可见文案、包名/路径/bundle id/脚本/活跃文档；测试 fixture 与示例代码中的任意字符串保留 |
| 1.3 仓库顺序 | 先建空公开 releases 仓库（含初始 commit），再改主仓库名并更新 remote；draft release → 验证下载/验签 → publish；releases 仓库只放资产，不镜像源码 |
| 1.4 配置迁移 | 明确 macOS 实际路径 `~/Library/Application Support/ZMark`；迁移在 `load_or_default` 前执行；纯函数接受 old/new 根目录并测试三种分支 |
| 1.5 旧 App 删除 | 先检查进程并移动到 `target/bundle/GlideMark.previous.<ts>.app` 作为回滚副本，`lsregister -u` 旧 bundle id；新 App 启动验证通过后再删除 |
| 2.1 平台结论 | 增加 AppTranslocation、`/Volumes/`、不可写父目录检测；拒绝这些环境；签名在 staging 完成，安装后不再改 bundle 内容 |
| 3.1 Ed25519 缺口 | 新增直接依赖 `ring`、`serde_json`、`hex`；签名/公钥导出改用仓库内 Rust 工具（ring），不依赖系统 OpenSSL；`notes` 不展示，不进入签名载荷；发布脚本自动验签 |
| 4.1 重复下载 | `UpdateStatus::{Downloaded,Installing}` 记录已暂存版本；重启前不重复下载 |
| 4.2 网络 | 拆分 connect/max timeout、重试、最大体积、单飞、手动检查显示具体错误 |
| 4.3 版本比较 | 数值段比较、严格更高、拒绝降级/非法/预发布 |
| 4.4 启动/环境 | 更新器由 `main.rs` 启动，测试注入 no-op；非 `.app`/Translocation/只读环境禁用并给出原因；失败退避 |
| 4.5 展开校验 | 校验可执行文件、bundle id、版本、`codesign --verify`、解压根目录；清理 staging |
| 4.6 回滚 | `.old.<version>` 保留到下一次成功更新；提供手动恢复路径；首次启动不删除备份 |
| 4.7 首次安装 | 明确 v0.1.0 无自更新能力，v0.2.0 必须手动安装一次 |
| 5.1 导出测试 | 使用 `TestAppContext::simulate_new_path_selection` + `did_prompt_for_new_path` 做真实集成测试；建议文件名由纯函数覆盖 |
| 5.2 草稿文件名 | 使用本地化 `untitled_file` 去扩展名 + `.html`；不做“首标题”推断 |
| 5.3 写入线程 | 保存路径返回后在 `background_executor` 写文件；toast 使用 generation 防旧定时器清理新提示 |
| 5.4 快捷键 | 关于页快捷键改为 `⇧⌘E`；`reveal_path` 测试平台为 no-op，仅人工验收 |
| 6.1 logo | 使用 256px 图标显示 72pt；加入 `ASSETS` 并更新资产测试 |
| 6.2 版本 | app crate 升到 `0.2.0`；About 用 `APP_VERSION` 常量（`env!("CARGO_PKG_VERSION")`）；release.sh 校验 |
| 6.3 关于页更新按钮 | 更新状态归属 `WorkspaceView`，`SettingsWindowView` 已 observe workspace，读取并展示状态；按钮调用 `WorkspaceView::check_for_updates` |
| 7.1 阶段 | 拆为 6 个可独立验证阶段（见 §8） |
| 7.2 不可证伪声明 | 改为“staging 验证 + 重启时两步可恢复换位 + helper 自愈”；E2E 用本地 file:// feed，不发布假 v0.2.1 |
| 7.3 验收 | 补充精确命令、plist/codesign/lsregister/rename-grep/feed 验签检查 |
| 8.1 文档所有权 | 用户本轮明确要求全局改名，记录为对 AGENTS.md 文档所有权的用户豁免；README/GLOSSARY/AGENTS 活跃引用同步更新，历史 ADR/计划不改 |
| 8.3 `--skip-tests` | 删除该开关；release.sh 始终运行阶段门槛 |
| R1/N1 `.old` 第二次更新冲突 | 换位前先 `rm -rf` 同版本 `.old.<version>`；轮换更早备份；清理上次遗留 staging；helper 在目标缺失时恢复 `.old.*` |
| R2/N3 改名门禁误报 | 只保留一份门禁脚本：排除 `target/**`、`docs/adr/**`、`docs/plans/**`，再用显式 allowlist 过滤合法残留（README 旧名说明、config 的 `LEGACY_APP_DIR`、旧 App 清理路径、`GLIDEMARK_BIN`），只有非 allowlist 命中才失败 |
| R3 更新器注入波及 70 处构造 | 保留 `WorkspaceView::new(state, cx)` 为默认禁用入口；新增 `new_with_update_config` 供 `main.rs` 使用；现有测试不改签名 |
| R4 缺少 quarantine 清理 | staging 校验后、换位前执行 `xattr -dr com.apple.quarantine`，换位后再校验一次 |
| N2 draft→publish 命令不完整 | `gh release edit v<version> --repo 2716190547/zmark-releases --draft=false`；publish 前用 `gh release download` 读草稿资产，publish 后再用公开 URL 终验 |
| N4 “原子换位”不严格 | 明确为“两步可恢复换位”：先备份到 `.old.<version>`，失败反向恢复；helper 在目标缺失时恢复；不宣称严格原子 |
| N5 `cx.restart()` 路径不可控 | 不依赖 GPUI restart；新增显式 restart helper（等待 PID 后 `open <实际目标 bundle>`），再 `cx.quit()`；S5 人工断言 |
| N6 bundle 脚本非原子安装 | 检测运行中的 ZMark；先把现有包移到备份再复制，失败恢复；不 `rm -rf` 运行中的 bundle |
| N7 签名接口/notes 歧义 | `notes` 本轮不展示、不进 UI；`release_tool sign --payload-file <file>`，避免换行参数 |
| N8 staging 遗留 | 每次解压前清理旧 staging |
| O1 门禁 allowlist 漏测试 fixture/示例 | 门禁排除 `crates/*/tests/**`（crate 导入由编译器保证），并用 `scripts/rename_allowlist.txt` 显式登记 `syntax.rs` 示例行、README 旧名说明等；README 文案固定为含 `formerly GlideMark` |
| O2 目标缺失时无法“启动自愈” | 自愈移入 restart helper：等 PID 退出后若目标 bundle 缺失，先恢复最近 `.old.*` 再 `open`；测试直接执行 helper 脚本（stub `open`） |
| O3 helper 硬编码 `/Applications` | helper 接收实际目标路径；staging 始终在目标父目录；环境检测只要求父目录可写 |
| O4 同版本 `.old.<version>` 嵌套 | 换位前 `rm -rf "目标.old.<当前版本>"`；更早备份可带时间戳；补“同版本失败重试”测试 |
| O5 `.previous` 嵌套 | `bundle_macos.sh` 换位前先删除/轮换 `.previous`（带时间戳），恢复后校验是合法 bundle |
| O6 文档残留旧方案 | 统一 §0/B1/B2/7.2/S5/§8 为“两步可恢复换位 + 显式 helper + `cx.quit()`”；把 `.old` 全部改为 `.old.<version>` |
| O7 取消关闭语义 | 明确顺序：只有脏文档全部处理完才换位并启动 helper；取消关闭提示 => 保留 staging，不换位、不启动 helper |
| O8 签名/生成器/持久化/pgrep | `verify --payload-file`；`release_tool manifest` 确定性生成 JSON；`staged.json` 持久化已下载版本；`/usr/bin/pgrep` 区分未运行与命令失败 |
| O1（v4 审查）门禁自匹配 | `rg` 增加 `--glob '!scripts/check_rename.sh'`、`--glob '!scripts/rename_allowlist.txt'`；门禁加 `--self-test`（干净树 0、fixture-only 0、非 allowlist 1） |
| O2（v4 审查）helper 启动晚于换位 | 换位全部移入 helper：脏文档处理完后**先启动 helper 再 `cx.quit()`**；helper 等待 PID 退出后执行换位/缺失恢复/`open`；App 内不再做 `mv`；测试模拟“第一步 mv 后进程退出” |
| O3（v4 审查）manifest 签名写回未定义 | `release_tool manifest ... --signature <hex>` 直接输出最终 JSON；`notes` 可选（`serde(default)`）；补 manifest→sign→verify→App 解析端到端测试 |
| O4（v4 审查）次要项 | §8 文案改为“可写任意位置（通常 /Applications）”；旧 App 备份改带时间戳并先清理；补导出不可写路径测试；`staged.json` 在版本回退/目标缺失/版本不符时清理 |
| O1（v5 审查）helper 未轮换旧 `.old.*` | helper 在换位前循环删除非当前版本的 `.old.*`；补连续三次更新测试 |
| O2（v5 审查）失败分支不重新打开旧 App | 两条失败路径在恢复目标后 `open "$target"` 并写日志；S5 覆盖两次 mv 失败 |
| O3（v5 审查）helper 被终止的窗口 | 安装 `trap restore EXIT INT TERM`；成功前清除 trap；SIGKILL/断电需手动恢复，§8 如实收窄 |
| O4（v5 审查）allowlist 漏旧备份时间戳 | allowlist 增加 `GlideMark.previous.*.app`；加入 `--self-test` 的 fixture-only 用例 |
| O5（v5 审查）文档一致性 | §3.2 `manifest` 补 `--signature <hex>`；syntax.rs 用稳定代码片段而非行号；§8 修正自愈边界 |
| O1（v6 审查）恢复失败后删备份 | 目标缺失且恢复失败时 `[ -d "$target" ] || exit 1`，保留全部 `.old.*`；轮换移到第一次 `mv` 成功之后；补“恢复失败保留备份”测试 |
| O2（v6 审查）trap 不恢复更早备份 | `restore` 在 `backup` 不存在时回退到最近的 `.old.*`；补 SIGTERM 时序测试 |
| O3（v6 审查）缺 trap 测试 | S5 增加 helper 在两次 mv 之间收到 TERM 的测试，断言旧包被移回并可 open（stub） |
| O4（v6 审查）文档项 | §0 标题更新为 v1→v8；§8 说明恢复失败保留 `.old.*` 供手动恢复 |
| O1（v7 审查）INT/TERM trap 不终止 | `trap restore EXIT` 与 `trap 'trap - EXIT; restore; open "$target" || true; exit 1' INT TERM` 分离；SIGTERM 后不再执行第二次 `mv` |
| O2（v7 审查）保留备份测试语义 | 测试用 stub `mv` 让所有移动都失败，断言 `.old.*` 全部保留；正常恢复失败路径只断言“未删除备份” |
| O4（v7 审查）SIGTERM 断言 | S5 断言 helper 非 0 退出、staging 未嵌套进 target、target 仍为旧 bundle、open stub 被调用 |

## 1. 全局改名 ZMark

### 1.1 命名规则

| 对象 | 旧 | 新 |
| :--- | :--- | :--- |
| 产品/显示名 | GlideMark | **ZMark** |
| 应用包 | GlideMark.app | **ZMark.app** |
| 可执行文件 | GlideMark | **ZMark** |
| bundle id | com.glidemark.app | **com.zmark.app** |
| core crate / 目录 | glidemark_core / crates/glidemark_core | **zmark_core / crates/zmark_core** |
| app crate / 目录 | glidemark_app / crates/glidemark_app | **zmark_app / crates/zmark_app** |
| 配置目录 | `~/Library/Application Support/GlideMark/` | **`~/Library/Application Support/ZMark/`** |
| GitHub 仓库 | 2716190547/GlideMark（私有） | **2716190547/zmark** |
| 发布仓库 | 无 | **2716190547/zmark-releases（公开，仅资产）** |
| Gitee | 无 remote | 不改（不存在） |

### 1.2 完整改名清单

代码与构建：
- `Cargo.toml` workspace members、`crates/*/Cargo.toml` 包名、path 依赖、`test-observation = ["zmark_core/test-observation"]`。
- `Cargo.lock`（已跟踪）由 cargo 重新生成并提交。
- 54 个测试文件与 `examples/phase_j_memory_probe.rs` 的 `use glidemark_*`。
- `[[test]]` target 名称与 `required-features` 保持不变，只改 crate 导入。
- `scripts/bundle_macos.sh`、`bench_memory.sh`（`GLIDEMARK_BIN` → `ZMARK_BIN`，保留旧变量一个版本的兼容读取）、`profile_fps.sh`。
- `.gitignore` 的 helpers 路径。
- 运行时临时目录：`state.rs:463` 的 `glidemark_test_*`、`workspace.rs` 中写死 `glidemark` 的临时路径 → `zmark_*`。
- i18n `app_title` 10 处、`settings.rs` 关于页/设置窗标题、`workspace.rs` 设置窗标题、`main.rs` 示例文档/窗口标题。
- 产品身份相关注释与文档字符串；测试 fixture 任意文本保留。

文档：
- README、GLOSSARY、AGENTS 中活跃的 GlideMark 引用。
- 历史 `docs/adr/*` 与已完成 `docs/plans/*` 不改写；新增文档使用 ZMark。

### 1.3 机械验收门

只保留一份 `scripts/check_rename.sh`（实现时新增），规则：

1. `rg -n 'glidemark|GlideMark' --glob '!target/**' --glob '!docs/adr/**' --glob '!docs/plans/**' --glob '!scripts/check_rename.sh' --glob '!scripts/rename_allowlist.txt'` 收集命中；`rg` 无命中（exit 1）视为通过。门禁自身与 allowlist 文件不参与扫描。
2. 过滤显式 allowlist（`scripts/rename_allowlist.txt`，每行含路径模式与原因）：
   - `crates/*/tests/**`：测试 fixture 任意文本保留（§1.2）；crate 导入是否正确由编译器保证，不靠 grep；
   - `README.md` 中含固定短语 `formerly GlideMark` 的迁移说明行（实现时按此文案写）；
   - `crates/zmark_core/src/config.rs` 的 `LEGACY_APP_DIR` / `"GlideMark"`（配置迁移兼容）；
   - `crates/zmark_core/src/syntax.rs` 的示例常量（按 allowlist 文件中的稳定代码片段登记，不使用会漂移的行号）；
   - `scripts/bundle_macos.sh` 的旧 App 清理路径（`GlideMark.app`、`GlideMark.previous.*.app`）；
   - `scripts/bench_memory.sh` 的 `GLIDEMARK_BIN` 兼容读取；
   - `crates/zmark_app/src/updater.rs` 的旧 App 路径清理常量。
3. 过滤后仍有任何命中 → 脚本 exit 1 并打印；否则 exit 0。
4. `scripts/check_rename.sh --self-test`：用临时目录构造三种树（干净改名 / 仅 fixture 残留 / 非 allowlist 残留），分别断言 exit 0 / 0 / 1；该自测纳入 S1 验收。

`docs/plans/**` 与 `docs/adr/**` 整体排除，因此新计划与历史文档中的旧名不会误报；普通 `rg` 命令不再作为门禁。



### 1.4 配置迁移

- `default_config_dir()` 返回 `dirs::config_dir()/ZMark`（macOS 即 `~/Library/Application Support/ZMark`）。
- 迁移在 `Preferences::load_or_default()` 之前调用：若新 `config.toml` 不存在且旧 `GlideMark/config.toml` 存在，复制到新目录；不删除旧文件。
- 抽成 `migrate_config_if_needed(old_root, new_root)` 纯函数，测试：新配置已存在不覆盖、旧配置不存在返回默认、复制失败继续默认。
- 旧 App 在新 App 首次启动后尽快从 `/Applications` 移除，避免两版本交替写配置。

### 1.5 安装与仓库顺序

1. 建公开空仓库 `zmark-releases`（`gh repo create ... --public --add-readme`），确认 `visibility=PUBLIC`。
2. 改主仓库名 `gh repo rename zmark`，立即更新本机 `origin` 并 `git fetch` 验证；不依赖 GitHub 重定向。
3. 完成改名与构建，先本地验证。
4. 用 draft release 上传资产，验证下载与验签后再 publish。
5. releases 仓库只放 zip/latest.json，不镜像源码。

### 1.6 旧安装清理

- `bundle_macos.sh` 安装 `/Applications/ZMark.app` 时：用 `/usr/bin/pgrep -x ZMark` 检测进程（exit 0=运行中则中止；exit 1=未运行；其他=报错中止）；否则先把现有包 `mv` 到带时间戳的 `ZMark.app.previous.<ts>`（换位前清理/轮换旧的 `.previous.*`，避免嵌套），`ditto`/`cp` 新包并校验签名，成功后再删除备份，失败恢复并校验恢复目标是合法 bundle。
- 新 `ZMark.app` 构建、签名、启动验证通过后，把 `/Applications/GlideMark.app` 移到带时间戳的 `target/bundle/GlideMark.previous.<ts>.app`（移动前清理同名旧副本），`lsregister -u` 旧路径，再删除副本。
- 若旧 App 正在运行，先不删除；提示用户退出后由人工/脚本完成。

## 2. HTML 导出交互

### 2.1 当前问题

`ExportHtml` 直接写 `doc.path.with_extension("html")`，无对话框、无反馈；草稿无路径时什么都不做。

### 2.2 新行为

1. `export_path_for_document(doc_path, untitled_file, home)` 纯函数：
   - 有路径：目录 = 文档目录，文件名 = `<stem>.html`。
   - 草稿：目录 = home，文件名 = `<本地化 untitled_file 去扩展名>.html`。
2. 调用 `cx.prompt_for_new_path(&suggested_dir, Some(&suggested_name))`；取消无副作用。
3. 在 `background_executor` 写 HTML（生成字符串可在主线程，写盘放后台），完成后回主线程显示成功提示。
4. 成功提示：顶部 3 秒 toast“已导出 <文件名>”+“在 Finder 中显示”（`cx.reveal_path`）；generation 计数避免连续导出时旧 timer 清掉新提示。
5. 失败：进入现有错误横幅；测试平台无真实 Finder，`reveal_path` 仅人工验收。
6. 关于页快捷键修正为 `⇧⌘E`。

### 2.3 测试

- 纯函数覆盖有路径/草稿/空标题/路径分隔符清洗。
- 集成：`cx.simulate_new_path_selection(|_| Some(path))` 后触发动作，断言文件存在与 toast 状态；`did_prompt_for_new_path()` 为真；取消（返回 None）不写文件。
- 保留 `export_test.rs` 内容断言。

## 3. 内置自动更新

### 3.1 通道与 manifest

- Feed：`https://github.com/2716190547/zmark-releases/releases/latest/download/latest.json`
- 资产：`ZMark-<version>.zip`（内含签名后的 `ZMark.app`）+ `latest.json`。
- manifest 字段：`version`、`url`、`sha256`、`published_at`、`notes`、`signature`。
- 签名载荷固定字节串：`version\nurl\nsha256\npublished_at`（无尾随换行）；`notes` 本轮不展示、不进入 UI，避免未验证内容出现。

### 3.2 密钥与签名工具

- 新增 `crates/release_tool`（workspace 成员，不进入 App 包）：
  - `generate`：ring `Ed25519KeyPair::generate_pkcs8` 生成密钥；私钥写 `~/.config/zmark/release.key`（chmod 600），公钥 hex 输出。
  - `pubkey`：从私钥导出公钥。
  - `sign --payload-file <file>`：从文件读取载荷，输出 64 字节签名 hex（避免 shell 换行参数问题）。
  - `verify --payload-file <file> --sig <hex> --pubkey <hex>`：发布前自检。
  - `manifest --version ... --url ... --sha256 ... --published-at ... [--notes ...] --signature <hex>`：确定性生成含签名的最终 `latest.json`（不靠 shell 拼字符串）。
- App 内嵌公钥常量（32 字节 hex）到 `updater.rs`；用 `ring::signature::UnparsedPublicKey` 验证。
- 直接依赖：`zmark_app` 增加 `ring`、`serde_json`、`hex`；`release_tool` 增加 `ring`、`hex`。
- 私钥丢失说明：旧版本将无法验证新发布；要求备份，且不提交。
- 不使用系统 OpenSSL（macOS `/usr/bin/openssl` 为 LibreSSL，不保证 `-rawin`）。

### 3.3 发布脚本

`scripts/release.sh <version>`（无 `--skip-tests`）：
1. 校验 `<version>` 等于 `crates/zmark_app/Cargo.toml` 版本、工作区干净。
2. 跑阶段门槛：`cargo test --workspace`、观测专项、Phase H 21 项（命令见 §7）。
3. `./scripts/bundle_macos.sh`：先在 `target/bundle/ZMark.app` 上 `codesign --force --deep --sign -` 并 `codesign --verify --deep --strict`，再复制到 `/Applications`。
4. 压缩 `target/bundle/ZMark.app` → `target/release/ZMark-<version>.zip`；解压到临时目录再次验签。
5. 计算 sha256；用 `release_tool sign --payload-file` 得到签名；用 `release_tool manifest ... --signature <hex>` 输出最终 `latest.json`（含 `signature`；`notes` 可选，缺省为空）；用 `verify --payload-file` 自检，并用 App 的 manifest 解析器反序列化 + 验签一次。`manifest` 的字段顺序固定，`signature` 由该命令写入，不靠 shell 拼接。
6. `gh release create v<version> --draft --repo 2716190547/zmark-releases` 上传两资产。
7. 用 `gh release download v<version> --repo 2716190547/zmark-releases --pattern 'ZMark-*.zip' --pattern latest.json --dir <tmp>` 读取草稿资产，本地验签并校验 zip SHA-256。
8. `gh release edit v<version> --repo 2716190547/zmark-releases --draft=false`；随即用公开 `releases/latest/download/latest.json` 与 manifest 内 zip URL 做最终验签/哈希检查。

### 3.4 App 更新服务

新增 `crates/zmark_app/src/updater.rs`，状态归属 `WorkspaceView`（`UpdateService` 字段或独立实体）：

- **启动**：保留 `WorkspaceView::new(state, cx)` 作为“更新默认禁用”的兼容入口（现有 70 处调用与测试不改签名）；新增 `new_with_update_config(state, UpdateConfig, cx)` 供 `main.rs` 使用。测试通过 no-op 配置注入，不在 `WorkspaceView::new` 无条件启动。
- **环境检测**：仅当 `current_exe()` 位于 `X.app/Contents/MacOS/`，且不在 AppTranslocation（路径含 `AppTranslocation`）、不在 `/Volumes/`、父目录可写时启用；否则禁用并在关于页显示原因。
- **检查**：启动后一次 + 每 24h 后台 timer；单飞（同一时刻只有一个检查/下载任务）。
- **网络**：`curl --connect-timeout 10 --max-time 300 --retry 2 -fsSL`；限制 manifest/zip 最大体积；失败记录 stderr，自动检查静默，手动检查显示具体原因。
- **校验**：manifest 签名 → 版本数值段比较（严格更高；拒绝非法/降级/预发布）→ zip SHA-256 → 解压 → `ZMark.app/Contents/MacOS/ZMark` 存在、`CFBundleIdentifier=com.zmark.app`、`CFBundleShortVersionString` 等于 manifest、`codesign --verify --deep --strict` 通过、解压根只有 `ZMark.app`。
- **staging**：解压到目标 bundle 父目录下的隐藏目录（如 `<target_parent>/.ZMark.staging.<pid>`），保证与目标同卷；每次解压前清理旧 staging；解压后 `xattr -dr com.apple.quarantine <staging>/ZMark.app`，换位后再检查一次。已暂存版本写入配置目录的 `staged.json`（version、staging 路径、sha256），启动时校验后恢复为 `UpdateStatus::Downloaded`，跨会话不重复下载。
- **安装（重启时）**：用户点“重启更新”：
  1. 若当前有脏文档，走 `on_window_should_close` 的现有提示；设置 `pending_restart=true`。**取消关闭提示 => 清除 `pending_restart`，保留 staging，不启动 helper。**
  2. 在 `after_dirty_doc_resolved` 的“全部处理完”分支、且 `pending_restart` 为真时：**先启动 helper，再 `cx.quit()`**。App 进程内不做任何 `mv`，换位完全由 helper 在进程退出后执行。
  3. helper（process_group 独立，参数为 target、PID、current_version、staging，日志写 `~/Library/Application Support/ZMark/update.log`）：
     ```sh
     target="$1"; pid="$2"; version="$3"; staging="$4"
     backup="$target.old.$version"
     while kill -0 "$pid" 2>/dev/null; do sleep 0.1; done
     restore() {
       if [ ! -d "$target" ]; then
         if [ -d "$backup" ]; then
           mv "$backup" "$target"
         else
           latest=$(ls -dt "$target".old.* 2>/dev/null | head -n1)
           [ -n "$latest" ] && mv "$latest" "$target"
         fi
       fi
     }
     trap restore EXIT
     trap 'trap - EXIT; restore; open "$target" 2>/dev/null || true; exit 1' INT TERM
     # 上一次两步换位在中间崩溃：目标缺失但备份还在
     if [ ! -d "$target" ]; then
       latest=$(ls -dt "$target".old.* 2>/dev/null | head -n1)
       [ -n "$latest" ] && mv "$latest" "$target"
     fi
     # 恢复不成功时保留全部备份，交给手动恢复；绝不先删备份
     [ -d "$target" ] || exit 1
     rm -rf "$backup"
     if ! mv "$target" "$backup"; then
       open "$target" 2>/dev/null || true
       exit 1
     fi
     # 目标已安全备份，再轮换更早备份，只保留当前版本
     for b in "$target".old.*; do
       [ "$b" = "$backup" ] || rm -rf "$b"
     done
     if ! mv "$staging/ZMark.app" "$target"; then
       mv "$backup" "$target"
       open "$target" 2>/dev/null || true
       exit 1
     fi
     trap - EXIT INT TERM
     xattr -dr com.apple.quarantine "$target"
     /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$target" 2>/dev/null || true
     open "$target"
     ```
  4. 由于 helper 在换位前已经运行，即使 App 在第一步 `mv` 后崩溃/被强制退出，helper 仍会完成换位或从 `.old.*` 恢复，覆盖“目标缺失”的真实窗口。helper 自身收到 SIGTERM/SIGINT 时，INT/TERM trap 先恢复、`open` 旧 App 并 `exit 1`，不会继续执行第二次 `mv`；SIGKILL/断电无法捕获，需手动把 `.old.<version>` 移回。
  5. 旧备份保留到下一次成功更新；提供手动恢复说明（把 `.old.<version>` 移回）。
- **UI**：顶部非阻塞横幅“ZMark vX 已就绪：重启更新 / 稍后”；`UpdateStatus` 包含 Idle/Checking/Latest/Error/Downloaded/Installing；关于页“检查更新”按钮显示同一状态。
- **失败退避**：网络/权限失败记录并退避，不在每次启动重复弹错；手动检查可重试。
- **首次安装**：v0.1.0 无更新器，v0.2.0 必须手动安装一次；写入发布说明。

### 3.5 测试

- 纯函数：版本比较、非法版本、降级、manifest 解析、签名载荷、hex 解码、单飞状态。
- 签名：ring 生成测试密钥对，验证通过/篡改/截断/非法 hex/哈希不符。
- 下载与安装流程：注入 `feed_path`、`staging_root`、`target_bundle`，在临时目录验证“manifest→下载（file://）→验签→解压→校验→两步换位→恢复”。不触碰真实 `/Applications`。
- 备份轮换：连续三次更新时只保留最近一份 `.old.<version>`，不产生嵌套；同版本失败重试先 `rm -rf` 同名备份；上次失败遗留的 staging 会被清理。
- 重启 helper：直接执行生成的 shell 脚本（`open` 用 stub/标记文件替代），覆盖“目标存在正常换位”“目标缺失先恢复 `.old.*`”“第一步 `mv` 后进程退出”“同版本重试”“第一次/第二次 `mv` 失败后重新 `open` 旧 App”“stub 使所有 `mv` 失败时 `.old.*` 全部保留”“两次 `mv` 之间收到 SIGTERM”七条路径；SIGTERM 用例断言 helper 非 0 退出、staging 未被嵌套到 target、target 仍为旧 bundle、open stub 被调用。真实时序测试用短生命周期 PID。
- manifest 端到端：`manifest → sign → manifest --signature → App 解析/验签` 全链测试，缺 `signature` 或 `notes` 缺省时 App 仍能解析（`serde(default)`）。
- 导出失败：注入不可写路径，断言错误进入错误横幅且不产生成功 toast。
- 跨会话：`staged.json` 校验通过后启动即显示“已下载”，不重复下载；若 `staged.version <= current`、staging 缺失或 bundle 版本不符则清理并重新检查。
- 更新状态：已暂存不重复下载、稍后不提示、手动检查状态转移。
- 环境：非 .app/Translocation/只读目标被拒绝并给出原因。
- 端到端人工：本地 file:// feed 上 v0.2.0 → 测试 v0.2.1，验证横幅/重启/版本变化；真实发布只发 v0.2.0。

## 4. 关于页

- 顶部 72pt logo：复制 `assets/icon.iconset/icon_256x256.png` 到 `crates/zmark_app/assets/brand/logo.png`，注册到 `ASSETS`，`img("brand/logo.png").w(px(72.)).h(px(72.))`。
- 右侧：**ZMark** / `GPU-Accelerated Markdown Editor & Reader · Rust + GPUI` / `Version <APP_VERSION>`。
- 增加“检查更新”按钮：状态来自 `WorkspaceView::update_status`；`SettingsWindowView` 已 `observe` workspace，在回调中同步状态并 `cx.notify()`；点击通过 `WeakEntity<WorkspaceView>` 调用 `check_for_updates`。
- 非 .app/Translocation/只读时按钮显示禁用原因。
- 快捷键清单修正 `⇧⌘E`。
- 测试：`ASSETS` 含 logo 且可加载；`APP_VERSION` 等于 `env!("CARGO_PKG_VERSION")`（不写源码 grep）；不自动断言 `reveal_path`。

## 5. 版本与构建

- `crates/zmark_app/Cargo.toml` 版本升到 **0.2.0**；`zmark_core` 同步 0.2.0。
- `APP_VERSION: &str = env!("CARGO_PKG_VERSION")` 供 About 使用。
- `bundle_macos.sh` 从 Cargo.toml 读取版本写入 `CFBundleShortVersionString`/`CFBundleVersion`。
- 首次发布 v0.2.0；不发布测试用的 v0.2.1。

## 6. 阶段拆分

1. **S1 改名 + 配置迁移 + bundle/版本**：crate/目录/导入/脚本/bundle id/版本 0.2.0/配置迁移测试/活跃文档；机械 grep 门禁；全量 + 观测 + Phase H + 打包。
2. **S2 导出交互**：纯函数 + 原生保存面板 + 后台写盘 + toast + 测试。
3. **S3 关于页**：logo/版本/检查更新按钮壳（状态先 from WorkspaceView）。
4. **S4 更新器纯逻辑**：manifest/版本/签名/网络封装/状态机 + 单测。
5. **S5 staging/安装/重启/横幅**：临时目录集成 + `pending_restart` 与脏文档流程 + 显式 restart helper（实际目标路径、自愈）+ `cx.quit()`。
6. **S6 发布闭环**：公网仓库、密钥、release.sh、v0.2.0、旧 App 清理、人工更新验收。

每阶段独立提交；阶段候选运行全量、观测专项与 Phase H 后交付。

## 7. 精确验收命令

```sh
# 默认全量
cargo test --workspace

# 观测专项（改名后）
cargo test -p zmark_app --features test-observation \
  --test phase_i_syntax_and_warm_palette_test \
  --test rework4_stage_a_workspace_test -- --test-threads=1

# Phase H 21 项串行
cargo test -p zmark_app --features test-observation \
  --test phase_h_b1_real_geometry \
  --test phase_h_b2_highlight_matches_copy \
  --test phase_h_b3_autoscroll_real_events \
  --test phase_h_b4_cmd_c_routing -- --test-threads=1

# 改名残留（唯一门禁；含 allowlist 与退出码判定）
./scripts/check_rename.sh

# bundle 校验
/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' /Applications/ZMark.app/Contents/Info.plist
/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' /Applications/ZMark.app/Contents/Info.plist
codesign --verify --deep --strict /Applications/ZMark.app
codesign -dv --verbose=2 /Applications/ZMark.app

# 发布源
gh repo view 2716190547/zmark-releases --json visibility
curl -fsSL https://github.com/2716190547/zmark-releases/releases/latest/download/latest.json
# 用内嵌公钥验签 + 校验 zip sha256
```

## 8. 风险与取舍

- 改名不可逆；历史文档保留旧名，README 说明旧名来源。
- Ad-Hoc 签名无法提供系统级信任；系统策略为“仅 App Store”时仍可能被拦。Ed25519 只保证更新包完整性与来源。
- 两步可恢复换位依赖同卷 `mv`；`.old.<version>` 保留到下一次更新，避免新版本崩溃后无法回退；目标缺失由 restart helper 在正常终止/SIGTERM 时自愈；恢复失败时 helper 保留全部 `.old.*`，由用户手动移回；SIGKILL/断电也需手动恢复。
- AppTranslocation/只读位置拒绝更新；用户需把 App 放在父目录可写的任意位置（通常是 `/Applications`）。
- 国内访问 GitHub 可能慢/失败；自动检查静默、手动检查显示原因，后续可加 Gitee 镜像。
- 私钥丢失会使现有用户无法验证新版本；必须备份且不入库。
- 新 i18n 文案需补全全部现有语言；测试数量与断言不因改名而减少。
- 文档所有权：本轮用户明确要求全局改名，README/GLOSSARY/AGENTS 的活跃引用同步修改，记录为用户豁免；历史 ADR/计划不改写。

## 9. 独立审查记录

- 首轮审查（2026-11-06）：结论“有异议”，阻塞项 B1-B3、应修 1.1-8.3 已逐条处理并写入 §0。
- 第二轮复审（v2）：仍“有异议”；新增/遗留 R1-R4、N1-N9（`.old` 冲突、draft→publish、门禁误报、构造函数注入、quarantine、换位非严格原子、restart 路径、bundle 脚本、签名接口、staging 遗留）。v3 已逐条处理并写入 §0。
- 第三轮复审（v3）：仍“有异议”；O1-O8（门禁 allowlist、目标缺失自愈、helper 硬编码路径、同版本 `.old` 嵌套、`.previous` 嵌套、旧文案、取消语义、签名接口/生成器/持久化/pgrep）。v4 已逐条处理并写入 §0。
- 第四轮复审（v4）：仍“有异议”；O1-O4（门禁自匹配、helper 启动晚于换位、manifest 签名写回、次要文案/备份/导出失败/staged 清理）。v5 已逐条处理并写入 §0。
- 第五轮复审（v5）：仍“有异议”；O1-O5（helper 未轮换旧 `.old`、失败分支不重开旧 App、trap 缺失、allowlist 漏时间戳备份、文档一致性）。v6 已逐条处理并写入 §0。
- 第六轮复审（v6）：仍“有异议”；O1-O4（恢复失败后删备份、trap 不恢复旧 `.old`、缺 SIGTERM 测试、文档项）。v7 已逐条处理并写入 §0。
- 第七轮复审（v7）：仍“有异议”；O1-O4（INT/TERM trap 不终止、备份保留测试语义、SIGTERM 断言、文档项）。v8 已逐条处理并写入 §0。
- 第八轮复审（v8）：**结论“对该 revision 无未解决实质异议”**。审查版本：`docs/plans/zmark-rename-export-update-and-about.md`，SHA-256 `a654cf880321c3054d95bc71573e1a3ba34875e031663166e500d500d3eeaeaf`。交付时附上 `check_rename.sh --self-test`、S5 helper 专项、全量/观测/Phase H、bundle/codesign、draft→publish 验证记录。
