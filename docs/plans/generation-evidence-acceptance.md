# 生成任务模型事实与秘密回显验收（M03 增量）

日期：2026-11-18。先补生成任务可靠性，不把本切片当作模型纠正、视觉兼容或真实业务验收完成。复用模型路由、任务私有历史与现有执行槽，不引入依赖。

## 实现边界

1. 模型尝试在请求前写 started，在返回后原位写实际结果，future 取消写 cancelled；429、5xx、传输失败、超时和硬拒绝分别记录。started 表示本地准入意图，不能证明提供商已收到请求。
2. 模型 trace 提供有界全量 checkpoint 回调，在 trace 数据锁外运行。开始事实持久化失败不发请求；结束事实失败不继续备用，也不能展示成功。回调失败保持可诊断，错误不包含回调原文或提供商正文。
3. Web 生成任务沿 result 保存独立版本化证据，使用候选索引引用提交时冻结的 model snapshot；不改写不可变输入。紧凑尝试元组保留可读结果名称和字段说明，最大 201 次调用、4 候选必须保持在现有 32 KiB 限额内。记录生成应用结果，区分模型调用成功与领域文档失败。
4. 先更新证据，再发布业务终态；已有任务历史周期/终态写入与重启恢复继续生效。对正在调用时的强杀，只保留已持久化 started 意图及 interrupted，不推断远端完成、不重放。取消结果仅表示本地 future 已退出，不能撤销远端已发生的请求。
5. 服务在解析后、任何业务名称日志或落盘前检查整个生成批次的已解码字段；命中本任务冻结的任一非空凭据则固定错误拒绝。critic 同样检查。不能替换 YAML、放宽断言或调用备用掩盖秘密。旧配置主/备用和注册候选都进入本任务秘密集合；任意已废弃/未知秘密不声称能够识别。
6. 成功来源沿用原有 CaseProvenance；失败/取消从任务 result 读取调用证据。旧配置路由的内部 provider 尝试不能凭候选推断，必须明确未记录；M03 在这部分与模型纠正闭环完成前不勾选。

## 顺序和验收

- 核心：开始/结束 checkpoint 故障、取消 RAII、真实 loopback 429/5xx/transport、总时限与硬拒绝；来源校验接受新增可重试结果，拒绝把 started/cancelled 当完成来源。
- 生成：密钥在早/晚用例、Unicode/JSON 转义、YAML/name/critic 字段中时无原文落盘，普通有效输出保持 Draft。
- 真实 HTTP：发生实际请求的失败、无效生成文档、备用成功、请求中取消、critic 未验证、重启保留和最大证据尺寸。扫描本次隔离任务/资产/日志，排除本来就保存秘密的私有模型注册库/配置。
- 工作区 Rust、Clippy、受影响 HTTP、独立复核；四平台 Release 与 Pages 预览分别核验。保留历史 fmt/Clippy 与真实项目环境缺口，不用 fixture 代替业务成功。

## 本机证据

`cargo test --workspace --offline`：294 项通过（Web 44、core 248、集成 2），另 1 项需显式运行时归档而忽略。Clippy、UI 19 项、严格 TypeScript/工作台及 Pages 构建、actionlint、66 份 Markdown/285 个相对链接均通过；Clippy 既有告警与工作台 chunk 大小提示保留。独立复核通过。

真实服务入口 `python3 scripts/test-generation-evidence.py /绝对路径/qinaegis` 验证失败/备用、应用校验、critic 未验证、秘密回显、请求前事实、取消和强杀恢复；模型协议、基础服务、契约生成/执行、任务历史、流程/流程执行七组 HTTP 回归共同通过。新脚本已接入四平台候选 CI。

实际 Chromium 验证免费 beUI 选择与折叠控件、主/备用实际四次调用、冻结候选、critic 未验证、取消及中断说明；刷新后从持久任务重新读取事实，不将中断显示为生成中，已知 fixture 凭据未进入页面。控制台 0 errors/0 warnings，证据在本机 `output/playwright/generation-*.png`。本机系统代理曾转发回环夹具并将断连转换为 5xx；Rust 的 loopback 测试现在显式 no_proxy，HTTP 头按名称大小写不敏感、值精确相等校验，生产代理行为未修改，原有失败分类和安全断言保持。

2026-11-18 后续：旧配置生成的严格主备与真实尝试也已实现并通过 `scripts/test-legacy-generation.py`，旧历史保持 not_recorded。模型纠正按[独立方案](model-correction-acceptance.md)完成本机验收；不复用 generator/critic 调用序列冒充 correct 依据。最新范围以验收清单为准。
