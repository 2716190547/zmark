# 版本与发布

当前版本：**0.2.16**。更新说明：[v0.2.16](v0.2.16.md)。

从 0.2.13 起，安装包、签名更新清单和发布说明统一发布在公开源码仓库 [`2716190547/zmark` 的 Releases](https://github.com/2716190547/zmark/releases)。v0.2.12 及更早版本留在已归档的 [`2716190547/zmark-releases`](https://github.com/2716190547/zmark-releases/releases)，以保留旧下载链接，详见 [ADR 0038](../adr/0038-move-releases-into-the-source-repository.md)。`dev` 是当前开发分支，Release 标签由 `scripts/release.sh` 固定到实际构建的已推送 source commit。

## 版本来源

- `crates/zmark_app/Cargo.toml`、`crates/zmark_core/Cargo.toml`、`crates/release_tool/Cargo.toml` 和对应的 `Cargo.lock` 项使用同一版本。
- About 从 app 的 `CARGO_PKG_VERSION` 读取；macOS `Info.plist`、CI 安装包名和 Windows NSIS `VERSION` 从 app manifest 生成，无需另改常量。
- 同步中英文 README 的当前版本和说明链接；新增 `docs/releases/v<version>.md`。历史计划、历史测量和第三方依赖版本不随应用版本替换。
- `rust-toolchain.toml` 与 CI 固定 Rust 1.100.0；使用 rustup 的 Cargo，避免 PATH 中旧 Cargo 覆盖工具链选择。

## 验证与推送

本机体验迭代按用户约定不运行测试，直接构建安装。准备推送构建更新时，对最终候选执行：

```sh
scripts/full_gate.sh --bundle
bash scripts/test_release_assets.sh
```

门禁覆盖 workspace 测试、串行的 Phase H/I 六目标、rename 检查及 macOS bundle。完成自行审查后提交并推送至 `dev`（按用户指令同步合入 `master`），GitHub Actions `build-desktop` 生成三个平台的产物。

## 公开发行

仅使用同一次成功 CI、同一 source commit 的产物。将 Actions 产物下载到 `target/` 下的独立目录后直接传入脚本（**无需额外下载 `.sha256` 校验文件或回下载产物核对 SHA-256**；macOS 也可在同 source commit 下由 `release.sh` 直接调用 `./scripts/bundle_macos.sh` 打包），执行：

```sh
ZMARK_MACOS_ZIP=/absolute/path/ZMark-<version>-macos.zip \
ZMARK_WINDOWS_SETUP=/absolute/path/ZMark-<version>-setup.exe \
ZMARK_LINUX_ASSETS_DIR=/absolute/path/linux-artifacts \
./scripts/release.sh <version>
```

默认发布到 `2716190547/zmark`；`ZMARK_RELEASES_REPO` 仅在 0.2.13 过渡发布时临时指向归档前的旧仓库。发布脚本会再次运行测试门禁，核验 macOS 签名和版本，创建 DMG、仅基于本地待上传产物计算 SHA-256 并签名 macOS/Windows 更新清单（供应用内自动更新验签使用），随后直接创建公开发行并用 GitHub API 核对资产条目完整即可，**不再回下载草稿或公开下载产物二次比对 SHA-256**。发布说明默认读取 `docs/releases/v<version>.md`；可用 `ZMARK_RELEASE_NOTES` 指定文件。签名密钥保留在仓库外，禁止输出或提交。

Gitee 镜像由本仓库 `.github/workflows/mirror-gitee.yml` 在 Release 发布后同步，需要仓库 Secret `GITEE_RELEASE_TOKEN`；镜像仓库保留 `zeng_sc/zmark-releases` 原名，因为已安装客户端的国内备用地址写死了它。镜像保证同步 macOS ZIP、Windows 安装程序和两份签名更新清单，DMG 与 Linux 包以 GitHub 为准。镜像上传使用 runner 预装的 curl；**无需回下载核对已上传或既有资产的 SHA-256**。GitHub 发布成功不代表镜像已经更新。发布记录须写明 source commit、CI run、各测试结果与发布资产条目确认结果。CI 通过不能替代 Windows/Linux 桌面实机体验验收。
