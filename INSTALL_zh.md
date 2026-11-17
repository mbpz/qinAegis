# 安装本地 Web 服务

2.0 正在开发。下列发布包安装步骤仅适用于**对应版本经过验证并正式发布后**；发布前请按 [README](README_zh.md) 从源码构建。Homebrew Formula 当前为发布模板，不应直接安装。

`codex/qinaegis-2.0` 分支和 PR 的自动构建会在 Actions 运行产物中提供服务候选归档。核对工作流提交、目标平台和 manifest，再验证摘要。候选包不代表签名、干净机器安装或完整 2.0 GA 已通过；已有 `v1.0` 桌面发布不能当作 2.0 安装包。

## macOS / Linux

从可信 2.0 发布页下载并审阅 `install.sh`：

```bash
sh install.sh --version 2.0.0
```

脚本识别 macOS ARM64/Intel 或 Linux x86_64，下载对应服务归档及 `SHA256SUMS`，校验后安装至 `~/.local/bin/qinaegis`，启动服务并打开浏览器。不使用 sudo、不修改 shell 配置、不删除系统安全标记。自行将目录加入 PATH，或使用绝对路径。SHA-256 依赖可信发布渠道，并非独立发布者签名。

支持 `--prefix /绝对路径/bin`、`--no-start`，离线安装使用 `--archive /路径/包.tar.gz --checksum 十六进制摘要`。升级前先停止旧服务。启动失败时检查 status 与服务日志，不能将文件安装完成等同于服务就绪。

```bash
~/.local/bin/qinaegis status
~/.local/bin/qinaegis open
~/.local/bin/qinaegis stop
~/.local/bin/qinaegis serve --no-open
```

## Windows

下载 `qinaegis-x86_64-pc-windows-msvc.zip`，通过 `Get-FileHash` 与发布 `SHA256SUMS` 核对，解压到当前用户目录并运行 `qinaegis.exe`。本阶段尚无一键 PowerShell 安装器。签名及干净系统验收是发布前条件，不通过关闭系统安全机制解决未签名产物问题。

## Playwright + Midscene.js 引擎

基础包尚不自动安装引擎。浏览资产和设置不需要 Node；实际执行视觉测试需要兼容 Node、Playwright/Midscene.js 及自动化浏览器。

安装式运行时目录须包含以下清单及真实配套文件：

```json
{"schema_version":1,"engine":"midscene","version":"your-verified-engine-version","entry":"dist/executor.js","node":"bin/node"}
```

将示例 `version` 替换为已验证的引擎版本，仅使用 ASCII 字母、数字、点、连字符或下划线。`entry` 和可选 `node` 指向目录内文件；只有清单无法运行。配齐编译后的执行器、锁定生产依赖和兼容浏览器，在启动服务前设置 `QINAEGIS_RUNTIME_DIR` 为该绝对目录。修改环境变量后重启服务。

开发时可准备仓库根 `sandbox` 的依赖与浏览器，再设置 `QINAEGIS_DEV_SANDBOX` 为源码绝对目录；开发适配器使用 `src/executor.ts` 和 `node_modules/tsx/dist/cli.mjs`。不隐式回退编译时源码目录。自动下载/修复引擎和完整 Web 引擎管理属于后续阶段。

已安装兼容的 Node 24 时，在 2.0 源码目录准备开发引擎：

```bash
cd sandbox
npx --yes pnpm@11.24.0 install --frozen-lockfile
npx --yes pnpm@11.24.0 run build
npx --yes pnpm@11.24.0 exec playwright install chromium
export QINAEGIS_DEV_SANDBOX="$PWD"
cd ..
./target/release/qinaegis
```

Linux 还可能需要 Playwright 文档要求的浏览器系统依赖。执行 AI 动作前在本机 Web 工作台配置兼容视觉模型。官方 Allure HTML 使用同一引擎及固定 `allure@3.33.0`，生成时不调用模型、不需要 Java 服务，但仍需要 Node 依赖准备完成；下载报告不会自动安装视觉引擎。


## 独立运行时候选包

构建机准备 Node 24 与 pnpm 11.24.0，运行 `python3 scripts/build-runtime.py`，在 `build/runtime` 生成独立归档与摘要；基础服务归档不包含引擎。核对可信 SHA-256、停止服务后本地导入：

```bash
python3 scripts/install-runtime.py /绝对路径/runtime.tar.gz --sha256 可信SHA256 --data-dir /绝对路径/私有数据
```

后续服务命令设置同一 `QINAEGIS_DATA_DIR`。安装器校验归档边界、宿主、内容和摘要，暂存后切换 `runtimes/midscene/current`，旧目录保留为 `previous`。服务活动或已有备份需人工决定保留时明确拒绝；不能只凭摘要标记跳过文件完整性检查。该导入工具当前仍需要本机 Python，完整一键 Web 分发尚待实现。

**Environment** 可检测运行时和浏览器，明确授权 **Install Chromium** 后安装匹配浏览器，并显示实际启动结果。下载使用选定运行时缓存，不提权安装系统库。真实模型兼容与干净机器验收另行完成。

## 数据与卸载

默认项目保留在 `~/.qinAegis/projects`，全局配置路径由操作系统决定。启动前设置绝对路径 `QINAEGIS_DATA_DIR` 可将项目、契约、模型注册库、服务状态、默认运行时及 `config.toml` 隔离到同一根目录；空值/相对路径会被拒绝。修改前先停止旧服务，后续命令使用相同设置；不会自动迁移已有资产，显式运行时/状态目录仍优先。先停止服务，再移除安装的 `qinaegis` 程序即可卸载基础包，默认保留配置与项目。需要清理数据时先备份；替换程序不等于回滚未来的数据格式迁移。

日常操作见[用户指南](docs/USER_GUIDE.md)，完整入口见[文档索引](docs/README.md)，候选包与正式发布边界见 [CI 交付说明](docs/CI_INTEGRATION.md)。
