# QinAegis 项目全景分析报告

> 生成日期: 2026-05-14 | 版本: v0.5.4 | 作者: AI Agent (DeepSeek)

---

## 目录

1. [架构总览](#一架构总览)
2. [项目完成度评估](#二项目完成度评估)
3. [竞品对比分析](#三竞品对比分析)
4. [差异化定位总结](#四差异化定位总结)
5. [未来演进方向建议](#五未来演进方向建议)
6. [结论](#六结论)

---

## 一、架构总览

```
┌─────────────────────────────────────────────────────────────┐
│              QinAegis Desktop GUI (macOS)                    │
│         tao + wry WebView ── React + TypeScript             │
│  ┌──────────┬──────────┬──────────┬──────────┬──────────┐  │
│  │Dashboard │ Explore  │ Generate │  Run     │ Reports  │  │
│  │          │          │          │          │          │  │
│  │ Settings │ Review   │ (InitWizard)                      │  │
│  └──────────┴──────────┴──────────┴──────────┴──────────┘  │
└────────────────────────┬────────────────────────────────────┘
                         │ RPC Bridge
┌────────────────────────┴────────────────────────────────────┐
│              Rust Core Services (crates/core)                │
│  ┌───────────┬──────────┬──────────┬──────────┬──────────┐  │
│  │ Explorer  │Generator │ Critic   │ Executor │ Healer   │  │
│  ├───────────┼──────────┼──────────┼──────────┼──────────┤  │
│  │ Reporter  │ Gate     │Knowledge │ Storage  │ Config   │  │
│  ├───────────┼──────────┼──────────┼──────────┼──────────┤  │
│  │Performance│ Stress   │ Protocol │ LLM      │ Sandbox  │  │
│  └───────────┴──────────┴──────────┴──────────┴──────────┘  │
└────────────────────────┬────────────────────────────────────┘
                         │ JSON-RPC (stdio)
┌────────────────────────┴────────────────────────────────────┐
│         Node.js Sandbox (sandbox/)                           │
│  ┌──────────────┬──────────────┬──────────────┐             │
│  │ executor.ts  │ explorer.ts │yaml_runner.ts│             │
│  ├──────────────┼──────────────┼──────────────┤             │
│  │lighthouse.ts │locust_runner │action_cache  │             │
│  └──────────────┴──────┬───────┴──────────────┘             │
│                        │ CDP                                 │
│              ┌─────────▼─────────┐                          │
│              │  Playwright +     │                          │
│              │  Midscene.js      │                          │
│              └───────────────────┘                          │
└─────────────────────────────────────────────────────────────┘
                         │
              ┌──────────▼──────────┐
              │  Local FS Storage   │
              │  ~/.qinAegis/       │
              │  projects/{name}/   │
              │  ├── spec/          │
              │  ├── cases/         │
              │  │   ├── draft/     │
              │  │   ├── reviewed/  │
              │  │   ├── approved/  │
              │  │   ├── flaky/     │
              │  │   └── archived/  │
              │  ├── reports/       │
              │  └── knowledge/     │
              └─────────────────────┘
```

### 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| **桌面 GUI** | Rust + tao + wry | WebView 原生壳 |
| **前端 UI** | React + TypeScript + Vite | 9 个功能视图 |
| **核心服务** | Rust + tokio | 业务逻辑编排 |
| **沙箱层** | Node.js + Midscene.js | AI 浏览器操作 |
| **浏览器** | Playwright (Chromium) | CDP 模式 |
| **AI 引擎** | Midscene.js (视觉) | explore/act/assert/extract |
| **存储** | 本地文件系统 | ~/.qinAegis/projects/ |
| **分发** | Homebrew Cask | brew install 一键安装 |

### 开源依赖

- **[web-infra-dev/midscene](https://github.com/web-infra-dev/midscene)** — AI 视觉执行引擎 (~6k stars)
- **[microsoft/playwright](https://github.com/microsoft/playwright)** — 浏览器自动化底座 (~70k stars)
- **[GoogleChrome/lighthouse-ci](https://github.com/GoogleChrome/lighthouse-ci)** — 性能门禁
- **[locustio/locust](https://github.com/locustio/locust)** — 压力测试

---

## 二、项目完成度评估

### 2.1 模块实现矩阵

| 模块 | 状态 | 说明 |
|------|------|------|
| **explorer.rs** | ✅ 完成 | BfsExplorer + MidsceneAutomation, Auth 支持 |
| **generator.rs** | ✅ 完成 | LLM 驱动的 YAML 测试用例生成, i18n prompt |
| **critic.rs** | ✅ 完成 | AI 审核评分, 覆盖率/断言/风险评估 |
| **executor.rs** | ✅ 完成 | 并行执行 (Semaphore), Self-Healing 集成 |
| **healer.rs** | ✅ 完成 | LLM 自动修复失败步骤, 保留原始 YAML |
| **reporter.rs** | ✅ 完成 | HTML/Markdown/JSON 报告生成 |
| **gate.rs** | ✅ 完成 | E2E + Performance + Stress 统一门禁, 表格/JSON 输出 |
| **knowledge.rs** | ✅ 完成 | 失败分类 (product_bug/test_issue/env/model), 抖动追踪, 覆盖率 |
| **performance.rs** | ✅ 完成 | Lighthouse 指标模型 + Baseline 对比 |
| **stress.rs** | ✅ 完成 | Locust 压测结果模型 |
| **protocol.rs** | ✅ 完成 | JSON-RPC 2.0 Rust ↔ Node.js 双向通信 |
| **llm.rs** | ✅ 完成 | MiniMax VL + OpenAI 兼容, ArcLlmClient |
| **storage/** | ✅ 完成 | Trait + LocalStorage 实现, 5 态生命周期 (draft/reviewed/approved/flaky/archived) |
| **automation/** | ✅ 完成 | BrowserAutomation trait + Midscene 实现 |
| **config/app.rs** | ✅ 完成 | 全局 + 项目级配置, env var 解析 |
| **service/** | ✅ 完成 | TestCaseService |
| **prompts/** | ✅ 完成 | i18n 系统/用户提示词模板 |
| **sandbox/adapter.rs** | ✅ 完成 | PlaywrightBrowserAdapter |
| **web_client (GUI)** | ✅ 完成 | tao + wry + React, 9 个视图组件 |
| **sandbox (Node.js)** | ✅ 完成 | executor, explorer, yaml_runner, lighthouse, locust, action_cache |
| **Homebrew Formula** | ✅ 完成 | qinAegis.rb + GitHub Actions CI |
| **GitHub Actions CI** | ✅ 完成 | release.yml, publish-homebrew-tap.yml, deploy-pages.yml |

### 2.2 Phase 完成情况

| Phase | 内容 | 实现状态 |
|-------|------|----------|
| **Phase 1** | Bootstrap: 配置 + 沙箱搭建 | ✅ 已实现 (转向本地存储, 放弃 Notion) |
| **Phase 2** | AI Core: 视觉驱动执行引擎 | ✅ 已实现 (Midscene + BFS Explorer) |
| **Phase 3** | Test Execution: 四类测试 | ✅ 已实现 (Smoke/Functional/Performance/Stress) |
| **Phase 4** | 本地数据模型 | ✅ 已实现 (LocalStorage + 5 态生命周期) |
| **Phase 5** | Distribution: Homebrew Cask | ✅ 已实现 (DMG 打包 + brew install) |
| **Week 13+** | Self-Healing, Action Caching, Review UI | ✅ 已实现 |
| **移动端** | iOS/Android 测试 | ❌ 未开始 (Phase 4 规划) |
| **集成扩展** | OWASP ZAP, Stagehand, Testplane | ❌ 文档已有, 代码未集成 |

### 2.3 总体完成度: ~85%

核心闭环完整 (Explore → Generate → Review → Run → Report → Gate), 但扩展集成和跨平台覆盖有缺口。

---

## 三、竞品对比分析

### 3.1 开源项目对标

| 项目 | Stars | 定位 | 与 qinAegis 差异 |
|------|-------|------|-------------------|
| **[browser-use](https://github.com/browser-use/browser-use)** | ~93k | LLM-native 浏览器 Agent | 通用 Agent, 无测试资产治理, 无 GUI, 无质量门禁 |
| **[stagehand](https://github.com/browserbase/stagehand)** | ~8k | AI 浏览器操作抽象层 (SDK) | SDK 定位, 无测试生命周期, 无报告/门禁 |
| **[midscene](https://github.com/web-infra-dev/midscene)** | ~6k | 视觉驱动 E2E 测试引擎 | qinAegis 的引擎层依赖, 无平台层能力 |
| **[shortest](https://github.com/antiwork/shortest)** | ~4k | 自然语言 E2E 测试 DSL | CLI only, 无本地资产治理, 无性能/压测 |
| **[lost-pixel](https://github.com/lost-pixel/lost-pixel)** | ~4k | 视觉回归测试 | 单点能力, 无 E2E/性能/压测统一 |
| **[playwright-ai-qa-agent](https://github.com/username/playwright-ai-qa-agent)** | — | Self-Healing QA Agent | 单点 Self-Healing, 无平台闭环 |
| **[steel-browser](https://github.com/steel-dev/steel-browser)** | — | 浏览器沙箱基础设施 | 基础设施层, qinAegis 已用 Playwright 替代 |

### 3.2 商业产品对标

| 产品 | 核心能力 | qinAegis 差异 |
|------|----------|---------------|
| **mabl** | AI 生成 + Self-Healing + 持续测试 | 本地优先, 开源免费, 无 SaaS 锁定 |
| **Testim** | AI 定位 + 自愈 + 低代码 | 资产可 Git 版本化, 本地审计 |
| **Applitools** | AI 视觉回归 + Ultrafast Grid | 不限于视觉, 统一 E2E + 性能 + 压测门禁 |
| **Cypress Cloud** | 失败回放 + Flaky 管理 | 本地 evidence, AI 驱动的失败自动分类 |
| **BrowserStack** | 云端设备网格 | 本地沙箱优先, 远期可加 remote executor |
| **Lighthouse CI** | 性能预算断言 | 统一门禁 (E2E + Perf + Stress), 不仅是性能 |
| **Tricentis Tosca** | 模型驱动测试 + 企业治理 | 开源轻量替代, 本地知识库 |

### 3.3 多维度对比矩阵

| 维度 | qinAegis | browser-use | stagehand | shortest | mabl | Testim |
|------|----------|-------------|-----------|----------|------|--------|
| **桌面 GUI** | ✅ | ❌ (CLI/Python) | ❌ (SDK) | ❌ (CLI) | ✅ (SaaS) | ✅ (SaaS) |
| **本地优先** | ✅ 完全本地 | ✅ | ✅ | ✅ | ❌ SaaS | ❌ SaaS |
| **AI 探索** | ✅ BFS + Visual | ✅ Agent loop | ⚠️ partial | ❌ | ✅ | ✅ |
| **测试生成** | ✅ LLM → YAML | ❌ | ❌ | ✅ NL → test | ✅ | ✅ |
| **审核工作流** | ✅ 5 态生命周期 | ❌ | ❌ | ❌ | ✅ | ⚠️ |
| **Self-Healing** | ✅ LLM 修复 | ❌ | ⚠️ | ❌ | ✅ | ✅ |
| **Action Caching** | ✅ 内置 | ❌ | ✅ 内置 | ❌ | ✅ | ✅ |
| **质量门禁** | ✅ E2E+Perf+Stress | ❌ | ❌ | ❌ | ✅ | ✅ |
| **性能测试** | ✅ Lighthouse | ❌ | ❌ | ❌ | ⚠️ | ⚠️ |
| **压力测试** | ✅ Locust | ❌ | ❌ | ❌ | ❌ | ❌ |
| **失败知识库** | ✅ 自动分类+追踪 | ❌ | ❌ | ❌ | ✅ | ✅ |
| **一键安装** | ✅ brew cask | ⚠️ pip | ⚠️ npm | ⚠️ npm | N/A | N/A |
| **开源** | ✅ MIT | ✅ MIT | ✅ MIT | ✅ MIT | ❌ | ❌ |
| **跨平台** | ⚠️ macOS only | ✅ | ✅ | ✅ | ✅ Web | ✅ Web |
| **移动端** | ❌ (规划中) | ❌ | ❌ | ❌ | ✅ | ✅ |
| **安全扫描** | ❌ (文档已有) | ❌ | ❌ | ❌ | ✅ | ✅ |
| **Git 版本化** | ✅ 文件存储 | ❌ | ❌ | ⚠️ | ❌ | ❌ |
| **CI 集成** | ✅ exit code | ⚠️ | ⚠️ | ✅ | ✅ | ✅ |

---

## 四、差异化定位总结

qinAegis 的最佳对标表达:

> **"本地优先的 mabl/Testim + Playwright Test Agents + Cypress Test Replay + Lighthouse/k6 Gate" 的开源工程师版本**

### 核心壁垒

1. **唯一具有完整测试资产治理的开源 AI 测试平台**
   - draft → reviewed → approved → flaky → archived 5 态生命周期
   - 所有资产可 Git 版本化, 可审计

2. **唯一将 E2E + Performance + Stress 统一为质量门禁的开源工具**
   - 一个 exit code 判断是否可发布
   - gate.yaml 声明式阈值配置

3. **唯一具有本地失败知识库的 AI 测试工具**
   - 自动分类: product_bug / test_issue / environment / model_hallucination
   - 积累抖动指数, 覆盖率映射

4. **唯一提供桌面 GUI + brew 一键安装的 AI 测试平台**
   - 对标商业产品 (mabl/Testim) 的使用体验
   - 零门槛上手: 双击 → 配置 → 探索 → 测试

5. **100% 本地文件存储, 无 SaaS 锁定**
   - 所有数据在 `~/.qinAegis/`
   - 天然支持 Git 版本管理

### 差异化矩阵

```
                开源免费              商业SaaS
                 │                     │
    GUI ─────────┼── qinAegis ─────────┼── mabl/Testim
                 │                     │
    CLI/SDK ─────┼── browser-use       │
                 │   stagehand         │
                 │   shortest          │
                 │                     │
             单点工具              平台闭环
```

qinAegis 是位于左上象限 (GUI + 开源) 的唯一产品, 同时具有平台闭环能力 (右侧象限特征)。

---

## 五、未来演进方向建议

### 5.1 短期 (1-3 个月) — 巩固核心闭环

| 优先级 | 方向 | 理由 | 对标 |
|--------|------|------|------|
| **P0** | CI/CD 深度集成 (GitHub Actions, GitLab CI 模板) | 让 gate 直接阻断 PR, 从"开发者工具"升级为"团队基础设施" | Cypress Cloud, Lighthouse CI |
| **P0** | 报告增强 (失败对比视图, Timeline, 趋势图) | 提升失败复盘效率, 降低排查成本 | Cypress Test Replay |
| **P0** | Windows/Linux 桌面客户端 | 当前 macOS only 是最大用户基数天花板 | Playwright, Electron 生态 |
| **P1** | 集成文档落地 (OWASP ZAP, Stagehand, Testplane) | 文档已完成, 代码集成可大幅提升安全/视觉/稳定性覆盖 | OWASP, visual regression |
| **P1** | 知识库 AI 增强 (LLM 驱动的失败原因自动分析) | 从规则匹配升级到 AI 推理, 真正对标 mabl Auto-Heal | mabl Auto-Heal |
| **P1** | 测试覆盖率可视化 (页面 → 用例映射热力图) | 对标企业级测试治理 | Tricentis Tosca |

### 5.2 中期 (3-6 个月) — 扩展平台边界

| 优先级 | 方向 | 理由 | 对标 |
|--------|------|------|------|
| **P2** | 移动端测试 (iOS WebDriverAgent + Android ADB) | Phase 4 规划, 开源领域尚无完整对标产品 | BrowserStack, Firebase Test Lab |
| **P2** | Playwright Test Agents 集成 (planner/generator/healer) | 微软路线验证, 增强 AI 能力 | Playwright Test Agents |
| **P2** | 多项目仪表板 + 跨项目质量趋势 | 对标 SaaS Dashboard 体验 | mabl/Testim Dashboard |
| **P2** | 插件系统 (自定义 Reporter, 自定义 Gate Rule) | 企业级可扩展性, 社区贡献入口 | Cypress Plugin |
| **P3** | 视觉 Baseline 管理 (对标 Applitools) | Midscene 已支持视觉断言, 缺管理层 | Applitools Eyes |
| **P3** | Remote Executor Provider | 本地优先但可扩展云端, 团队共享 | BrowserStack, Sauce Labs |

### 5.3 长期 (6-12 个月) — 构建生态壁垒

| 优先级 | 方向 | 理由 |
|--------|------|------|
| **P4** | Git 原生协作 (PR 内嵌测试报告, Case Review 流程) | 工程师工作流深度整合 |
| **P4** | API 测试集成 (REST/GraphQL) | 前端 + API 全链路质量 |
| **P4** | 低代码测试编辑器 | 降低非开发人员使用门槛 |
| **P4** | 测试数据工厂 (AI 生成 Mock Data) | 对标 Functionize 测试数据管理 |
| **P5** | 社区模板市场 (测试用例模板, Gate 模板) | 生态建设, 降低上手成本 |

### 5.4 演进路线图

```
       现在                          3个月                           6个月                          12个月
    ┌────────┐                ┌──────────────┐               ┌──────────────┐               ┌──────────────┐
    │ 核心闭环 │ ──────────►   │  CI/CD 深集成  │ ──────────►  │  平台边界扩展  │ ──────────►  │   生态壁垒    │
    │         │                │              │               │              │               │              │
    │ ✅ GUI  │                │ GitHub Actions│               │ 移动端测试    │               │ Git 原生协作  │
    │ ✅ 探索 │                │ GitLab CI     │               │ PT Agents    │               │ API 测试      │
    │ ✅ 生成 │                │ 失败对比视图   │               │ 多项目仪表板   │               │ 低代码编辑器   │
    │ ✅ 审核 │                │ Windows/Linux │               │ 插件系统      │               │ 数据工厂      │
    │ ✅ 执行 │                │ ZAP 安全集成   │               │ 视觉 Baseline │               │ 模板市场      │
    │ ✅ 报告 │                │ AI 失败分析    │               │ Remote Exec   │               │              │
    │ ✅ 门禁 │                │ 覆盖率热力图   │               │              │               │              │
    │ ✅ 自愈 │                │              │               │              │               │              │
    │ ✅ 缓存 │                │              │               │              │               │              │
    └────────┘                └──────────────┘               └──────────────┘               └──────────────┘
```

### 5.5 与开源社区的对齐策略

| 社区项目 | 对齐方式 |
|----------|----------|
| **browser-use** (93k stars) | 借鉴 Agent loop 理念, 但不作为默认执行模式; 在 Explorer 模块中参考其多步任务编排 |
| **stagehand** | Action Caching 已借鉴; 远期可参考其多 LLM 切换机制 |
| **midscene** | 当前核心引擎层; 关注上游更新, 保持兼容 |
| **lost-pixel** | 参考其视觉 diff Dashboard 设计思路 |
| **Playwright ecosystem** | 接入 Playwright Trace, MCP, Test Agents 等周边能力 |

---

## 六、结论

### 当前状态

qinAegis 核心闭环完整度约 **85%**, 已经是一个可以独立使用的 AI 质量工程平台。相比同类开源项目, qinAegis 在"测试资产治理"、"统一质量门禁"、"本地知识库"三个维度形成了差异化壁垒。

### 最大机会点

1. **Windows/Linux 支持** — 当前 macOS only 是最明显的市场天花板, 跨平台后用户基数可 10x
2. **CI/CD 模板** — 让 gate 直接在 PR 中阻断, 是从"工具"到"基础设施"的关键一步
3. **移动端测试** — Phase 4 规划中的蓝海, 开源领域尚无完整对标产品

### 最大风险

1. **单人维护** — 项目 bus factor = 1, 需要社区贡献或团队扩展
2. **依赖 Midscene.js 上游** — 核心引擎非自研, 上游变更影响大
3. **市场认知** — AI 测试平台赛道拥挤 (browser-use 93k stars), 差异化需要时间沉淀和内容营销

### 建议优先级

```
P0 (立即):  CI/CD 集成模板 + Windows/Linux 客户端 + 报告增强
P1 (1-3月): 集成文档落地 + AI 失败分析 + 覆盖率可视化
P2 (3-6月): 移动端测试 + 插件系统 + 多项目仪表板
P3 (6-12月): 视觉 Baseline + Remote Executor + 低代码编辑器
```

---

*文档版本: v1.0*
*生成工具: DeepSeek Agent (deepseek-tui 0.8.27)*
*分析范围: 全量源代码 + roadmap + design docs + market comparison + GitHub 竞品调研*
