# QinAegis 2.0 文档索引

更新：2026-11-18。2.0 的核心资产是可复用、可维护、可修改、可纠正的契约；交付为本地服务和浏览器工作台，基于 Playwright + Midscene.js，使用官方 Allure 与免费 beUI。当前为开发版，完整目标尚未全部验收。

## 当前产品与使用

| 文档 | 用途与状态 |
|---|---|
| [README 中文](../README_zh.md) / [English](../README.md) | 产品定位、模块分工、源码启动、当前能力和限制 |
| [安装 中文](../INSTALL_zh.md) / [English](../INSTALL.md) | 候选包/正式包区别、用户级安装、运行时准备、卸载 |
| [用户指南](USER_GUIDE.md) | 当前 Web 操作、契约治理、模型、配置迁移、执行证据、Allure 与中断残留维护 |
| [产品记忆](../CONTEXT.md) | 用户确认的长期要求、架构约束和提交身份 |
| [完整产品方案](plans/local-web.md) | 2.0 全量目标、核心分工、交互、安全与 P0–P8 验收；含明确标注的改造前基线 |
| [三维能力复盘](plans/capability-map-review-2.0.md) | 用户拆解对照源码，测试分类/过程控制/资产治理、六组能力和修订目标 |
| [产品 Agent](agents/product.md) | 产品拆解、可信表述、目标/状态/依赖/指标和证据维护规则 |
| [验收 checklist](progress/acceptance-checklist-2.0.md) | 有依赖顺序的逐项完成条件、证据及外部条件 |
| [开发进度](progress/local-web-2.0.md) | 已实现与验证事实、剩余缺口；不能用完整方案代替此证据 |
| [路线图](../qinAegis-platform-roadmap.md) | 剩余顺序、阶段状态和完成条件 |
| [CI / 发布 / Pages](CI_INTEGRATION.md) | 四平台候选归档、摘要、草稿 Release、Homebrew 与预览/生产部署 |
| [变更记录](../CHANGELOG.md) | 2.0 未发布变更，以及明确隔离的历史记录 |

## 当前专项方案

| 文档 | 边界 |
|---|---|
| [契约执行追溯与生命周期验收](plans/contract-execution-acceptance.md) | 契约/生成/执行快照、审核迁移、Allure 与事实源 |
| [受控项目规格验收](plans/project-specification-acceptance.md) | F01 固定规格资产、冲突保护与冻结输入；真实浏览器验收通过 |
| [流程治理与计划验收](plans/flow-governance-acceptance.md) | F03 JSON 草稿、不可变发布和类型化快照；仅计划，实际浏览器与复核通过 |
| [流程实际执行路径](plans/flow-execution-acceptance.md) | F04/F05 已验收的 schema 2 等待/审核用例范围；自动视觉备用仍待实现 |
| [独立模型协议验收](plans/model-protocol-acceptance.md) | 原生 Messages 与批准的跨协议备用；本机协议及 Web 验证范围 |
| [生成调用事实验收](plans/generation-evidence-acceptance.md) | 注册链尝试、失败/取消/中断、秘密拒绝及 Tasks 查看；M03 仍有缺口 |
| [执行器目录清理边界](plans/engine-workspace-cleanup.md) | T06 所有权与错误传播；跨重启恢复仍待实现 |
| [原生运行时维护方案](plans/native-runtime-maintenance.md) | R03/R05 原生 Web 导入、升级与回滚；最小 Rust 库已获授权，完整交付验收仍有缺口 |
| [自动打包与官网迭代](plans/distribution-pages-2.0.md) | 分支候选包、独立 Pages 构建和正式发布边界 |

方案定义目标，进度记录事实；候选包 CI 通过不等于真实业务视觉验证、干净机器安装或完整 2.0 GA。公开网站不读取本机工作台数据。系统无障碍控制是规划中的可选能力，需要单独授权和平台验收。

## 协作与维护

- [领域边界](agents/domain.md)：契约、模型、执行、编排和证据的职责以及文档维护。
- [Issue 管理](agents/issue-tracker.md)、[标签](agents/triage-labels.md)：按实际范围和验收证据管理工作。
- [CLAUDE.md](../CLAUDE.md)：协作入口；仓库工作规范仍以 AGENTS.md 为准。

每次产品迭代需同步当前入口、操作说明、方案和验证证据。明确“已实现”“待实现”“待真实环境验收”，禁止只替换版本号或把提案写成可用能力。

## 历史设计、决策与评审

以下内容保留原始时间、代码示例、任务勾选和结论，文件顶部说明 2.0 的沿用/替代关系。旧 TUI、tao/wry、Notion、DMG/Cask、全 Rust 引擎和自动修复提案不覆盖当前产品；第三方比较及示例未经本轮重新调研或执行。历史文中的 `Completed` 只适用于其原阶段。

| 资料 | 内容与使用方式 |
|---|---|
| [ADR](adr/) | 六项接口/配置决策；存储和适配边界沿用并演进，以顶部状态及实际源码为准 |
| [早期设计](superpowers/specs/) | 14 份早期架构、执行、存储、门禁、成熟度与原生引擎提案 |
| [早期实施计划](superpowers/plans/) | 10 份阶段计划；原勾选不代表 2.0 验收 |
| [历史代码评审](../reviews/code/) | 两份历史提交评审，不能用于证明当前分支质量 |
| [项目分析报告](project-analysis-report.md) | 旧版本盘点，保留历史分析范围 |
| [测试平台比较](market-validated-testing-platforms-comparison.md) | 历史市场比较，不作为第三方当前能力或版本依据 |
| [Midscene](integrations/midscene.md) | 历史集成讨论；2.0 明确保留 Playwright + Midscene.js，实际兼容仍需验收 |
| [Playwright agents](integrations/playwright-test-agents.md)、[Stagehand](integrations/stagehand.md)、[Testplane](integrations/testplane.md)、[OWASP ZAP](integrations/owasp-zap.md) | 历史/候选集成，不代表已接入当前工作台或稳定可用 |

当前产品冲突时按用户最新要求和产品记忆处理，进度中保留实际证据与限制。历史文档提供决策背景，不承担当前安装和操作指引。
