# QinAegis 2.0 项目协作入口

当前产品定义以 [CONTEXT.md](CONTEXT.md) 为准；文档导航见 [docs/README.md](docs/README.md)。

- 核心是可复用、可维护、可修改、可纠正的测试契约。领域、项目绑定、模型、执行器、编排及证据分工解耦；修改不覆盖历史运行依据。
- 交付为本机 HTTP 服务和浏览器 Web 工作台，不再打包桌面客户端。前端使用官方免费 beUI；保留 Playwright + Midscene.js，报告使用官方 Allure。
- 当前为 2.0 开发版。契约治理、持久任务/中断查看、注册模型调用事实、原生运行时维护、schema 2 等待/审核用例执行与报告已有实现；真实视觉策略、完整过程证据、暂停/继续/接管、干净平台及授权无障碍仍有缺口。
- [完整方案](docs/plans/local-web.md)描述目标，[开发进度](docs/progress/local-web-2.0.md)记录事实，[CI 与发布](docs/CI_INTEGRATION.md)定义候选包/草稿/正式发布边界。历史文档只作历史依据。
- 提交身份为 `ttbb <apples398@163.com>`，遵守 AGENTS.md 的 Lore 提交协议。用户已授权继续实施；真实数据迁移、系统权限和正式发布按各自授权边界处理。

## 协作约定

Issues 使用 GitHub Issues，见 [issue tracker](docs/agents/issue-tracker.md)。标签见 [triage labels](docs/agents/triage-labels.md)。领域文档约定见 [domain docs](docs/agents/domain.md)。产品需求、能力表述和任务目标/状态按[产品 Agent](docs/agents/product.md)与[三维复盘](docs/plans/capability-map-review-2.0.md)执行；验收清单为任务状态唯一执行台账。
