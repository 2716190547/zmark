# 草稿自动保存路径修复

修订：r1，2026-11-20。用户已确认本修订的行为及本机构建安装范围。

## 原因与约定

`WorkspaceView::trigger_auto_save_for` 的延迟回调仅判断 Modified，未排除没有物理路径的草稿；编辑欢迎页或新草稿后调用 `Document::save`，得到 `Cannot save untitled document without destination path`。手动保存入口已有另存为分支，无需重做。

- 自动保存仅处理有物理路径且 Modified 的文档，草稿不弹错误或选择路径对话框。
- 草稿保留内容、脏状态和撤销记录；手动保存时选择路径，取消不改变草稿。
- 首次保存成功后，后续编辑按现有机制自动保存。
- 有路径文件的删除、权限及冲突错误仍照常提示，不修改底层保存保护。

## 验收与自查

回归检查放入现有 `app_state/document_lifecycle_regression_test.rs`，覆盖新草稿及命名欢迎页、自动保存跳过、手动保存取消及成功、后续自动落盘。现有真实文件保存失败用例保留。

自行审查 r1（按项目单代理约定，非独立审查）：核对手动保存另存为分支及输入、图片引用修复等自动保存调用均经共享自动保存入口；路径检查应位于延迟执行时，避免只依据标题或过期调度状态；不将草稿错误改成底层保存成功。上述约束下无未解决实质异议。此为易逆的局部缺陷修复，不新增 ADR。

用户选择日常验证：运行 `./scripts/bundle_macos.sh --install`；测试未运行（按用户要求），回归留给后续门禁。截图为 Windows，当前只能构建验证 macOS，Windows 实机行为待验收。不推送、不发布。

## 实际交付验证

- `./scripts/bundle_macos.sh --install` 成功：release 编译、应用包签名及校验、安装到 `/Applications/ZMark.app`，版本 0.2.11（修复随 0.2.12 发布，发布门禁见 `docs/releases/v0.2.12.md`）。
- `git diff --check` 通过。构建仍有 vendor 未使用项、objc 宏 cfg 和 block future-incompatibility 警告，不在本轮修复范围。
- 回归测试未运行（按用户要求），未宣称 RED/GREEN 或 Windows 实机验证通过。后续可运行 `cargo test -p zmark_app --test app_state scratch_auto_save_waits_for_manual_save_as -- --test-threads=1`。
- 自查最终 diff：仅增加自动保存资格的物理路径检查，保留缓冲区同步、任务清理和真实文件错误提示；用户已有的 `AGENTS.md` 修改和三个未跟踪 SVG 不纳入提交。

