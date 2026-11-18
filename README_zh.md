# QinAegis 2.0

基于 **Playwright + Midscene.js 视觉驱动测试**的本地 AI 质量工作台。安装并运行 `qinaegis` 后自动打开默认浏览器，前端内嵌于服务，不再打包桌面客户端。

产品核心是可复用契约：业务意图、输入、前置条件、断言与证据要求独立于项目绑定、模型和执行器。契约可维护、修改、纠正与版本追溯；模型接入可扩展、可切换；能力按真实场景组合并校验执行时序。详见[项目记忆](CONTEXT.md)和 [2.0 实现计划](docs/plans/local-web.md)。

当前分支正在分阶段实施 2.0，并非所有规划已交付。本地 Web、契约生命周期与执行追溯、模型注册/路由及官方 Allure 报告已实现；项目模型路由、合成检测、配置恢复和持久任务/重启中断恢复已接入；真实业务视觉执行、完整运行时在线交付和干净机器安装仍待验收。无障碍电脑控制属于后续可选能力，需要用户明确授权，不会随安装自动启用。

Settings 已支持已知配置来源的预览迁移、显式凭据选择、原文冲突检查与备份；Reports 可检查并确认清理已验证归属的临时残留，保留运行证据和已完成报告。Windows 受管进程使用 Job Object，真实 Windows CI 已通过；逐引擎取消时限仍待验收。

任务采用一个执行槽和最多八个 FIFO 等待任务，提交时冻结输入，显示排队位置并可在执行前取消。重启将未完成任务标为 interrupted，不自动重放。浏览器执行器使用独立 Chromium profile 和私有工作目录；目录清理核对持有的目录身份，失败明确返回；Windows 新路径及跨重启残留恢复仍待验收。

Generate 已接入受控项目规格：固定项目 spec.md、128 KiB 上限、原文对照、明确保存确认及精确内容冲突检查；显式选用已保存规格后，生成与 critic 共用冻结内容和原契约断言。核心/HTTP/UI 回归及真实浏览器验收通过：创建/原文对照、确认失效、冲突留稿、重新读取及保存迟到响应的项目隔离。

Flows 将历史计划与 schema 2 执行分别管理。执行页可组合已审核用例及显式等待，校验类型化输入输出、有限条件、依赖时序、超时和清理。保存、发布、准备与运行逐次审核；入队固定 Approved 全文、契约原断言及步骤选定模型。持久步骤事实支持刷新与重启恢复，不自动重放。历史 v1 计划保持不可执行；视觉用例只尝试一次，未开放自动视觉降级或未审核修复。

Tasks 可查看注册路由的实际生成/critic 调用及冻结模型输入，保留批准备用的原因、取消与中断事实。模型响应成功与业务校验分别记录，未验证的 critic 不显示为已验证；模型回显本任务已知凭据时，保存受影响资产前拒绝。旧配置生成也已记录冻结主备的真实尝试，采用严格响应与取消规则；未记录的历史任务保留原状态。Contracts 可选择受管 run/case，经显式 `correct` 路由生成待审纠正，查看完整替代定义与冻结依据，再人工批准或拒绝。新版运行观测单独关联，不宣称语义验证通过。本机/协议范围与真实项目缺口见[验收清单](docs/progress/acceptance-checklist-2.0.md)。


Windows 候选包现加入用户级 PowerShell 安装器，支持归档核验、运行保护、上一版回滚及保留数据卸载。Environment 显示安全浏览器下载进度，刷新恢复安装任务。schema 2 执行定义和受审用例快照与仅规划的 v1 流程分别验收，Web 执行与持久证据正在完成最终验收。安装入口见[安装说明](INSTALL_zh.md)，状态见[验收 checklist](docs/progress/acceptance-checklist-2.0.md)。

## 核心分工与当前范围

同一发布契约可以绑定到两个项目，通过参数承接环境差异。纠正产生经审核的新修订，已有运行仍保留原输入与证据。模型可以建议修订，不能为了通过而自行批准放宽业务断言。

| 职责 | 当前实现与边界 |
|---|---|
| 契约治理 | 草稿/编辑/审核/发布、版本比较、纠正证据、弃用、派生、人工确认的绑定迁移、受控导入导出 |
| 模型路由 | 私有注册库、能力与数据授权、任务路由和冻结快照；Chat Completions 与原生 Anthropic Messages 文本/JSON 协议，生成使用批准的有界备用，视觉只用一个兼容主模型 |
| 测试执行 | 通过显式运行时边界接入 Playwright + Midscene.js；执行前固定已审核脚本与契约输入 |
| 验证与报告 | 不可变运行记录，生命周期失败阻断门禁，官方 Allure 生成/查看/下载；当前为单次运行报告 |
| 场景组合 | 独立 v1 计划与 schema 2 审核执行、免费 beUI 组合、类型化数据流、有界等待及用例/Allure 证据 |
| 可选电脑控制 | 规划经授权的系统无障碍适配器，须单独授权并通过平台验收 |

领域层管理契约与验证规则，模型适配器提出建议，执行适配器完成动作，报告保存事实。职责独立，切换模型或引擎不改写业务预期。前端统一使用官方 beUI 免费组件。

[能力复盘](docs/plans/capability-map-review-2.0.md)按测试分类、过程可控和资产独立对照源码。当前证据包括截图、失败步骤、结果及冻结输入；完整 Trace/Console/Network、可复核归因、暂停/继续/接管仍待补齐。本地服务不等于整个产品零依赖或模型无出网。[产品 Agent](docs/agents/product.md)负责维护目标与有证据的任务状态。

## 从源码运行

合并主分支前，2.0 开发源码位于 `codex/qinaegis-2.0`。先前正式版使用不同架构，不要通过通用的“最新版本”链接获取 2.0。

```bash
git clone --branch codex/qinaegis-2.0 https://github.com/mbpz/qinAegis.git
cd qinAegis
cd crates/web_client/ui
npm ci
npm run build
cd ../../..
cargo build --locked --release --bin qinaegis
./target/release/qinaegis
```

默认本机端口为 `19876`，启动时通过私有短期令牌建立浏览器会话，不要分享启动链接。尚未配置 AI 也可以浏览已有资产；在 Settings 配置模型后使用 AI 功能。

```bash
qinaegis status
qinaegis stop
qinaegis serve --no-open
qinaegis start --port 19877
```

未加入 PATH 时使用程序完整路径。关闭网页不会停止服务，通过 `qinaegis open` 重新打开已授权会话。

## 安装与测试引擎

见[安装说明](INSTALL_zh.md)。基础包包含已构建 UI，最终用户不需要 Rust 或前端开发环境；源码构建需要相应工具。**实际视觉测试仍需要已准备好的 Playwright + Midscene.js 引擎**、兼容 Node 和自动化浏览器。Environment 页面可原生导入可信运行时归档、预览并确认升级/回滚、显式查询兼容的稳定发布资产，并检查版本与真实浏览器启动。导入无需本机 Python 或全局 Node；Chromium 仍需另行明确授权安装。完整在线交付和干净环境仍待验收。

- `QINAEGIS_RUNTIME_DIR`：安装后的运行时绝对目录，包含 `runtime.json`、编译后的执行器及依赖。
- `QINAEGIS_DEV_SANDBOX`：显式指定已安装依赖的 sandbox 源码绝对目录，仅用于开发。
- 不依赖编译时开发者目录或当前工作目录查找执行器。
- 性能/压测需要已安装且通过验证的 Lighthouse/Locust 环境；缺指标明确失败。ZAP 在受管容器取消实现前不可用。

默认项目保留在 `~/.qinAegis/projects`，全局配置使用操作系统对应的配置目录，不再写死 macOS 路径。启动前可用绝对路径 `QINAEGIS_DATA_DIR` 隔离私有数据与配置，不自动迁移旧资产；修改前需停止旧服务，详见安装说明。AI 操作可能向所配置模型提供方发送被测页面内容；契约共享包不应包含凭据和私有测试数据。

## 发布方式

`bash scripts/build-release.sh TARGET` 构建前端及服务归档，目标包括 macOS 双架构、Linux x86_64、Windows x86_64，实际支持以 CI 与干净环境验收为准。不生成 `.app`、DMG 或 cask。分支/PR 构建保留候选包，版本匹配的 `v2.*` 标签通过检查后生成待审阅草稿发布。产物包含来源元数据和校验摘要；候选构建不能等同于已签名正式安装器。Homebrew Formula 仅跟随经过摘要验证的正式稳定发布，仓库模板不能在发布前作为可用安装源宣传。

## Allure 与官网

在 Reports 选择已保存运行，生成、查看或下载官方 **Allure 3.33.0** 单文件报告。生成报告使用已准备的 Node 引擎，不重新执行测试；用例通过不能掩盖执行基础设施异常。HTML 支持离线查看，附件可能包含项目输入、脚本和截图，分享前需审阅。

GitHub Pages 官网用于公开产品说明，本机工作台运行在自己的电脑上。官网在 `crates/web_client/ui` 中通过 `npm run build:pages` 独立构建至 `build/pages`；PR 提供预览产物，审阅后合并 `main` 才部署。维护和验收边界见 [CI/CD 集成](docs/CI_INTEGRATION.md)与[开发进度](docs/progress/local-web-2.0.md)。

## 文档入口

从[文档索引](docs/README.md)和[用户指南](docs/USER_GUIDE.md)开始。[产品方案](docs/plans/local-web.md)定义完整目标，[开发进度](docs/progress/local-web-2.0.md)记录实现事实，[验收 checklist](docs/progress/acceptance-checklist-2.0.md)列出逐项完成条件和顺序，[路线图](qinAegis-platform-roadmap.md)说明后续顺序。历史方案与评审均标为归档，不覆盖当前 2.0 定义。

[English](README.md) · [License](LICENSE)
