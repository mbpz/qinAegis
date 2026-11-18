# 独立模型协议验收（M05）

日期：2026-11-18。实现范围：在现有模型适配器工厂中增加 `anthropic_messages`，复用私有模型注册、项目路由、批准候选、调用事实和超时预算，不新增依赖。

## 协议边界

使用 Messages API 的 `/messages`、`x-api-key` 和顶层 system，固定 `anthropic-version: 2026-10-21`。该版本不支持 temperature；收到相应选项应明确拒绝。文本内容按 content blocks 解析，JSON 使用 `output_config.format`，不能发 Chat Completions 的 `response_format`。仅支持 text/json，不声明 vision/tools，不进入现有 Midscene 视觉执行路径。远端数据授权、端点修改时重新明确提供密钥、注册修订检查继续有效。

只接受完整 `end_turn` 文本响应；截断、拒绝、工具调用、未知内容和无效 JSON 不能当成成功。本地只验证 JSON 语法，schema 由请求中的上游约束实现；本实现不宣称含通用本地 JSON Schema 验证器，生成/纠正仍执行各自的领域文档校验。请求/响应有时限和体积边界，禁用重定向，不输出原始响应、密钥或含凭据的地址。鉴权、权限、参数和不支持能力明确失败；限流/配额及暂时服务错误只能按已有预先批准的候选和总预算处理，不自动改协议、地址、模型或断言。

## 证明范围

1. 核心真实 loopback HTTP 验证路径、认证和版本头、角色/system、文本/JSON 载荷、内容块与结束原因、错误/限流、超时、响应上限以及批准降级。
2. 真实服务验证保存、声明能力限制、合成 probe、生成/纠正路由、实际候选调用事实和密钥不进入任务/资产；配置切换不改变已开始任务。
3. 实际 Chromium 验证免费 beUI 协议选择、声明限制、密钥重填提示、保存/probe/路由、错误恢复和刷新；控制台检查。
4. 工作区测试、Clippy、TypeScript/UI 构建、四平台 CI 和候选摘要核验。

这些验证证明协议适配和本机服务行为，不证明某个付费账户、任意模型名称或真实视觉业务已通过。真实上游连通性和模型能力由用户保存配置后显式 probe；视觉仍由 M06 单独验收。

本机验收记录：工作区 Rust 224 项（新增 4 项 wire 测试）、Clippy、UI 14 项、严格 TypeScript、两项前端构建通过。真实 HTTP 验证原生生成/critic 和批准的跨协议备用、实际调用事实及鉴权拒绝。Chromium 验证协议切换、能力/family 限制、密钥清除/重新填写、保存和 probe 身份、检测锁定、视觉禁用、路由及刷新，控制台 0 errors/0 warnings。独立复核通过。M05 修订 `9dff2e0` 的 Release `44148136999` 四平台、engine-checks 和 assemble 全部通过；首次 ARM64 因托管 runner 容量不足取消，重试后实际执行通过。Pages `44148137089` 预览通过/生产部署跳过。下载候选独立核验 13 项 SHA 与四归档内外清单一致，来源 PR 合并提交 `81a922d6095b87d453a8e273ac6c727365c17d0a`。

## 官方依据

- [Messages API](https://platform.claude.com/docs/en/api/messages/create)
- [API versioning](https://platform.claude.com/docs/en/api/versioning)
- [Structured outputs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs)
- [API errors](https://platform.claude.com/docs/en/api/errors)


## Chat Completions 统一边界补齐（2026-11-18）

`openai_compatible` 同样要求完整输出：一条 assistant 文本和明确 `finish_reason: stop`。缺失/null 结束原因、截断、拒绝、工具/函数/非空音频或注释、未知内容、坏 JSON、超过 4 MiB 的响应均硬失败；标准 null 辅助字段及空 annotations 可接受。非 429 的 4xx 与重定向硬拒绝，只有 429、5xx 或传输故障允许按批准链备用。所有错误为固定安全说明，不带响应正文。

新增三项核心真实 wire 测试；模型组 23 项通过。扩展真实 HTTP 验证 21 种硬失败每次只发一个请求，429/503 的 generator/critic 各按批准链执行两次，原子来源与实际请求一致。其他成功协议 fixture 均加入明确完成标记。该切片不等于 M03 的失败事实、模型纠正与完整秘密扫描全部完成。
