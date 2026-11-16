# QinAegis 2.0 CI 与网站交付

2.0 的交付物是包含 Web 界面的本地服务可执行文件。自动构建不再生成桌面客户端、DMG 或 Cask。当前仍为开发版；构建成功不表示全部产品验收完成。

## 触发与权限

| 工作流 | 触发 | 结果 |
|---|---|---|
| `release.yml` | 向 main 的 PR、main / codex/qinaegis-2.0 推送、手动运行 | 四平台检查与候选归档；不会公开 Release |
| `release.yml` | `v2.*` tag | 先校验 tag 与源码版本，全部检查通过后创建草稿 Release |
| `deploy-pages.yml` | PR / 对应网站文件更新、手动运行 | 构建独立网站预览 artifact；仅 main 可部署 GitHub Pages |
| `publish-homebrew-tap.yml` | 正式稳定的 v2 Release 发布 | 校验归档摘要，更新 `mbpz/homebrew-qinAegis` Formula |
| `roadmap-check.yml` | 每周一 09:00 UTC、手动运行 | 真实构建/测试结果与失败日志，不再用文件存在推断产品完成 |

普通构建仅有 `contents: read`。创建草稿 Release 的 job 才有仓库写权限；Pages 部署权限局限在 main 部署 job。PR 不拥有部署或 tap 推送权限。同一分支重复提交可取消旧构建，tag 发布保留独立执行。

## 可复现构建与产物

UI 使用 `npm ci`，Rust 使用 `--locked`，执行引擎使用 Node 24 与锁定的 pnpm 11.24.0。四个平台分别为：

| Runner | 目标 | 归档 |
|---|---|---|
| macos-15 | aarch64-apple-darwin | qinaegis-aarch64-apple-darwin.tar.gz |
| macos-15-intel | x86_64-apple-darwin | qinaegis-x86_64-apple-darwin.tar.gz |
| ubuntu-22.04 | x86_64-unknown-linux-gnu | qinaegis-x86_64-unknown-linux-gnu.tar.gz |
| windows-2022 | x86_64-pc-windows-msvc | qinaegis-x86_64-pc-windows-msvc.zip |

每个归档只包含 `qinaegis`（Windows 为 `qinaegis.exe`）、LICENSE、release.json。清单记录版本、目标、源码提交和 `engine_included: false`。归档旁另附目标 JSON 和 `qinaegis-TARGET.sha256`（归档、JSON 两项），便于从 Actions 下载候选包后核对。每次 CI 的 assemble job 还汇总并复核四平台完整候选包，上传 `distribution-candidate`，内含安装脚本和总 `SHA256SUMS`。

归档只含服务和内嵌 UI。Node、Playwright 浏览器和 Midscene 执行器尚需单独准备；不能把网页打开视为自动化引擎安装完成。macOS/Linux 的用户目录安装脚本已有离线回归，Windows 当前为 ZIP 手工安装，尚无一键安装器。候选构建尚不包含签名、公证或全部系统干净机器验收。

矩阵执行 UI 回归、Rust workspace 回归、隔离数据目录的服务启动/认证/契约生成/执行测试。Linux 后续 job 安装锁定引擎，核验官方 Allure 报告通过已打包服务的完整生成和读取链路。确定性模型和协议 fixture 不等于真实业务视觉测试。

## 版本与发布

本地核验：

```bash
python3 scripts/release-assets.py check
python3 scripts/test-release-assets.py
bash scripts/build-release.sh aarch64-apple-darwin
```

准备下一版本使用 `bash scripts/bump-version.sh 2.1.0-rc.1`。脚本仅修改产品版本，保留引擎独立版本；不会提交、打 tag 或推送。修改仍需 review 和回归。

Cargo workspace、Cargo.lock 中产品包、UI package/lock、安装器默认版本必须一致。tag 必须与源码完全相同（例如 `v2.0.0`），不接受版本不符或宽泛 `v2`。assemble job 汇总四平台归档，核对所有清单来自同一源码提交，tag 发布复用该已验证候选包；`SHA256SUMS` 覆盖归档、清单和安装脚本。`install.sh` 的默认下载指向明确版本，避免取到旧桌面版 latest。

tag 仅生成草稿，外部可见稳定发布为独立发布动作。Homebrew 仅处理已公开、非预发布的稳定 v2；需要仓库配置 `HOMEBREW_TAP_TOKEN` 并拥有目标 tap 写权限。没有 token 不影响候选包、草稿 Release 或 Pages 构建。不能在正式发布前把 tap 安装标为可用。

## 独立 GitHub Pages 网站

网站从 `docs/landing/index.html` 和独立前端入口构建，复用免费 beUI 组件。它用于产品介绍与交付说明，不连接本机 RPC、不读取模型配置或项目数据。无统计脚本或远程字体。

```bash
cd crates/web_client/ui
npm ci
npm run build:pages
```

输出为仓库根目录 `build/pages`，与内嵌工作台资源分离；只上传编译后的 HTML/assets，不把 docs 中其他文件公开。相对资源路径支持 `/qinAegis/` 项目站点。PR 上传预览 artifact，main 构建后部署到现有 Pages 环境。开发分支的预览通过不表示公开站点已经更新。

页面必须保留开发状态、契约跨项目复用的示意标签、模型数据授权边界，以及运行时准备/无障碍规划的限制。不展示虚构用例数量、性能数据或离线模型承诺。

## 诊断与完成标准

Actions 中按失败步骤查看日志；artifact 缺失、tag 不符、归档混入其他文件、不同提交混包或 Allure 引擎异常均应失败，不能降级成绿色。周检查保留报告与逐项日志，失败也会上传，不自动创建重复 issue。

本轮分发与网站改造的完成证据需要同时包含本地回归、真实平台构建和 Pages 预览结果。完整 2.0 仍需按 `docs/progress/local-web-2.0.md` 验收真实双项目视觉执行、引擎安装与诊断、持久任务/恢复、场景编排及各系统干净安装。生产网站部署、稳定 Release 和 Homebrew 发布需分别核验，不用候选 artifact 替代。
