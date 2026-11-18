# QinAegis 2.0 用户指南

更新：2026-11-18。适用于当前 2.0 开发分支，**不代表完整 2.0 已正式发布**。QinAegis 的核心资产是可复用测试契约；本地 Web 工作台负责维护契约、项目绑定、模型路由、审核、执行和报告。视觉测试基于 **Playwright + Midscene.js**。

本文按当前源码中的页面和操作编写，保留英文界面名称方便查找。发布、平台和验收状态以[开发进度](progress/local-web-2.0.md)为准；长期产品边界见[产品记忆](../CONTEXT.md)。

## 1. 安装、启动与停止

先按 [README_zh.md](../README_zh.md) 从 2.0 源码构建，或按 [INSTALL_zh.md](../INSTALL_zh.md) 核对可信候选包/对应正式发布包。当前历史 `v1.0` 发布不属于 2.0，不能用历史桌面包代替本指南中的本地服务。基础服务内嵌 Web 界面，无需另行打包桌面客户端。

```bash
qinaegis
qinaegis status
qinaegis open
qinaegis stop
```

| 命令 | 用途 |
|---|---|
| `qinaegis` 或 `qinaegis start` | 启动后台服务，并请求系统默认浏览器打开工作台 |
| `qinaegis status` | 查看服务状态与实际地址 |
| `qinaegis open` | 为已运行服务创建新的浏览器授权入口 |
| `qinaegis stop` | 请求停止服务；仍有活动任务时会拒绝停止 |
| `qinaegis start --port 19877` | 指定端口启动；已运行实例需先停止才能改端口 |
| `qinaegis serve --no-open` | 前台运行且不自动打开浏览器，便于查看诊断；另一个终端可执行 `qinaegis open` |
| `qinaegis version` | 查看当前程序版本 |

程序不在 PATH 中时，使用安装目录中的完整路径；Windows 使用 `qinaegis.exe`。默认只监听 `127.0.0.1`，初始端口为 `19876`。默认端口占用时会有限尝试其他端口；显式指定端口不会悄悄换端口。以 `status` 返回的地址为准，同一状态目录只允许一个实例。

关闭浏览器不会停止服务或后台任务。重新访问请使用 `open`，不要把含启动令牌的地址发给别人。若系统未成功打开浏览器，先查看命令输出和状态，不要反复创建项目或删除配置。

## 2. 会话、数据与报告隔离

工作台是当前电脑用户的本地服务，不是面向团队公网访问的服务器。浏览器使用一次性启动授权、HttpOnly/SameSite Cookie 与 CSRF 保护；启动授权有效期为 60 秒，会话闲置 8 小时后失效。授权失败时重新运行 `qinaegis open`。

默认项目数据位于 `~/.qinAegis/projects`；契约库和模型注册库由本机服务管理。全局配置路径使用操作系统配置目录，不要假定各系统均为 `~/.config/qinAegis`。需要隔离工作区时，在启动前设置绝对路径 `QINAEGIS_DATA_DIR`；项目、契约、模型注册库、服务状态、默认运行时和 `config.toml` 均使用该根目录，显式状态/运行时路径仍优先。空值或相对路径会被拒绝。先停止旧实例再改变环境，并保持后续命令设置一致；此操作不会迁移旧数据。卸载前停止服务，移除程序默认保留资产；有意清理数据前先备份。

本地保存资产不意味着所有 AI 操作均离线。远程模型可能收到需求、页面内容、截图或其他任务数据。配置模型时逐个审阅数据接收方和授权；契约正文、参数、证据引用和共享包均不要放入密钥。导出的运行证据与报告含项目数据，分享前人工审查。

普通 HTML 报告在不授予脚本权限的 iframe 中展示。Allure 为交互展示允许报告脚本执行，但不授予同源权限，并限制外部联网；它不能访问工作台父页面。请保留这些隔离限制，显示受限不应通过放宽浏览器安全设置解决。

## 3. 创建项目与安排工作

在 **Dashboard** 的创建表单填写项目名、目标 URL 和技术栈，提交后在其他页面选择该项目。项目名属于本地资产标识，不使用目录分隔符、路径或隐藏目录名；目标 URL 使用有效 HTTP/HTTPS 地址。不要用同名项目覆盖已有数据。

推荐先建立和审核契约，再绑定项目、生成并审核用例，最后执行和查看报告。也可直接浏览已有资产，或在 Generate 中明确选择不使用契约；探索不是每次执行都必须重跑的前置操作。Flows 可组合已有 Approved 用例和显式等待；未登记的动作不能执行。

例如，同一份“登录后可见工作台”的契约可以分别绑定商城后台与供应商平台；两个项目使用各自目标和输入。只有业务意图与断言确实相同才复用；业务预期不同则应修订或派生，不能只为测试通过放宽断言。

## 4. Contracts：建立、复用和维护契约

### 4.1 创建草稿与发布

进入 **Contracts**，点击 **New draft**：

1. 填写 Contract ID、正整数 Version 和 Intent。
2. Inputs / outputs 填 JSON 对象，值为 `string`、`number`、`boolean` 或 `object` 类型名，例如 `{"target":"string"}`。这是类型声明，不是实际参数值。
3. Preconditions、assertions、required capabilities 每行一项；至少需要一条断言。
4. 点击 **Validate & save draft**。保存不等于发布。
5. 审查保存后的完整草稿，勾选人工发布确认，再点击 **Publish reviewed draft**。编辑内容与已保存草稿不一致时需要先重新保存。

发布版本不可原地改写。服务记录本地人工审核身份；模型生成内容不会自动成为已批准契约。字段和断言的格式校验也不等同于证明业务语义正确。

### 4.2 为多个项目绑定同一版本

在 **Published contract / version** 选择发布版本，在 **Project binding & impact references** 选择项目并填写 Typed parameters JSON，点击 **Save project binding**。

参数必须与声明的输入名称完全一致且类型匹配。例如输入声明 `{"target":"string"}` 对应参数可为 `{"target":"https://your-authorized-target.example"}`。对象类型不能用数组代替。另一个项目可引用同一发布版本并填写自己的参数。

绑定固定契约版本。纠正发布新版本不会自动迁移旧绑定；引用列表显示相关项目及其版本。契约绑定也不会自动覆盖项目目标或已生成用例，实际执行仍需审核对应脚本与来源。

### 4.3 比较版本与提出纠正

选定发布版本后，在 **Version history & comparison** 选择另一版本，逐字段检查差异。发现错误时点击 **Propose correction from this version**：

1. 修改替代定义；保留原 Contract ID 并推进版本。
2. 填写 Correction reason 和至少一条 Evidence reference，不包含秘密。
3. 点击 **Submit correction proposal**。该操作只提出待审建议。
4. 在 **Correction proposals** 阅读原定义、完整替代定义、原因、证据和受影响项目引用。
5. 勾选审核确认后，选择 **Approve & publish correction** 或 **Reject correction**。

批准纠正会发布新版本，原版本、原运行记录和原绑定保留。修复定位或时序问题与改变业务预期需要分别审查，不能用改期望掩盖被测系统缺陷。

#### 模型辅助纠正与新版观测

先在 Models 为 `correct` 配置具有文本能力、数据授权明确的已保存模型及备用链。该入口不借用 generate 或旧配置路由。

1. 选择最新发布版本，在 **Model correction suggestion** 选择 Evidence project、受管终态 run 和具有该完整发布来源的 case。历史无来源、跨项目、活动 run 或旧基线会被拒绝。
2. 填写不含凭据的说明（最多 8192 UTF-8 字节），检查选定冻结事实，确认向配置的纠正模型发送必要摘要，再点击 **Generate pending suggestion**。不默认发送截图或整份报告。修改说明/证据后需重新确认。
3. 查看任务状态、真实尝试和错误；可 **Cancel suggestion**。成功只产生 Pending 提案，保存原发布、选定执行依据、说明、模型路线/实际调用和模型自述局限。继续按上述完整定义审查流程批准或拒绝。已开始的原子提交不能撤销，取消不等于回滚已保存提案。
4. 批准发布后显式绑定/迁移项目，重新生成或维护并审核用例，执行新版本。在 **Associate a new v… run observation** 选择新版 run/case，检查事实并另行确认 **Record new-version observation**。旧 run 不能当新版依据；passed/failed/incomplete 只描述选定运行，语义仍为 `not_verified`。

提案及新版观测独立保存在契约库 `contracts/library.json`，不依赖有上限的任务历史。原始失败、旧发布和旧运行均保留。模型建议、人工批准、结构校验和运行通过不能互相替代；模型请求失败、修订冲突或凭据回显均不自动发布或重试治理写入。

### 4.4 弃用、派生与选定绑定迁移

在 **Contract lifecycle** 中填写原因，检查选定版本和受影响绑定，再勾选本次人工确认。

- **Deprecate selected version**：弃用所选版本，阻止新的绑定和以该版本进行新生成；不删除历史资产。
- **Create derived draft**：填写新的 New contract identity，创建独立身份的 v1 草稿；需要另行审核发布。适用于业务规则分叉。
- **Explicit binding migration**：选同一契约的非弃用目标版本；点击 **Prepare source bindings for review** 获取来源绑定列表，再编辑 Selected bindings JSON。仅保留本次要迁移的项目，为每个项目按目标输入类型填写完整参数。检查 Target inputs，重新确认后点击 **Migrate reviewed bindings**。

迁移只改选定项目绑定，不改已有用例和运行证据。需要应用新定义时，重新生成并人工审核新用例。更改选择、原因或参数会清除此前确认，请对实际提交内容重新审核。

### 4.5 受控导入与导出

**Export shared package** 下载发布契约交换包，不含项目绑定及本机生命周期历史。导出前仍应检查契约正文和证据引用是否含私有信息。

通过 **Choose local JSON** 选择不超过 1 MiB 的交换包，只会读入完整预览，不会立即导入。核对 schema、定义、断言、能力及外部审核来源，勾选本机人工导入确认，再点击 **Import reviewed package**。外部 reviewer 字段不是本机审核的替代；不能导入带项目 bindings 的共享包。

遇到 revision/conflict 时点击 **Refresh library**，比较刷新后的库与待导入内容，再重新确认。页面保留编辑输入，但旧确认不继续生效，不应通过重复点击绕过冲突检查。

### 4.6 Generate：受控项目规格

在 Generate 选择已有项目后，**Saved project specification** 读取该项目固定的 `spec.md`。缺失时可创建，最多 128 KiB；不能输入本机路径。通过 **Original saved specification** 对照原文，在 **Specification Markdown** 编辑，勾选 **I reviewed this specification and approve saving it** 后点击 **Create specification** 或 **Save specification**。改动内容会清除确认及选用状态。

保存使用读取时的精确内容修订；冲突会保留编辑草稿。需要丢弃当前草稿时明确点击 **Reload saved specification**，再比较当前内容重新确认。切换项目会重置编辑器和规格选用，不能沿用上一项目的迟到响应。

需要参与生成时，显式勾选 **Use this saved specification for generation**。有未保存改动时不能选用；保存后也需重新选择。服务在提交时校验修订并冻结规格，生成与 critic 共用规格及原契约断言；规格不能替换或放宽契约断言。后续修改不改变排队任务或历史生成来源。未选用时规格不参与该次生成。

当前核心、真实 HTTP、UI 和实际 Chromium 操作验收通过，F01 已勾选；包括保存迟到响应的项目隔离。完整条件见[规格验收方案](plans/project-specification-acceptance.md)。

### 4.7 Flows：执行组合与历史计划

**Flow workspace** 默认选择 **Execution flows**，以下历史说明可在 **Planning history** 查看。历史 v1 计划只准备快照，不执行动作；schema 2 执行定义独立保存和审核。页面采用已有免费 beUI 组件。

1. 在 **Saved flow draft** 选择草稿，或用 **Load planning template** 开始。编辑 **Flow draft JSON**，通过 **Original saved flow draft** 对照原文；输入来源只允许字面量、流程输入或上游输出。服务校验类型、依赖、端口、权限、副作用与非幂等重试限制。
2. 勾选保存确认，点击 **Save flow draft**。保存使用整库修订 CAS；冲突保留编辑输入。需要放弃编辑时明确点击 **Reload flows and discard editor draft**。保存后对相同内容点击 **Validate flow draft**，再勾选发布确认并点击 **Publish flow version**。编辑会清除旧确认；发布版本不能原地覆盖。
3. 在 **Project plan** 选择发布版本和已有项目，填写 **Typed flow inputs JSON**。浏览器 target 的字面量或流程输入必须匹配项目固定目标；契约参数来自该项目选定绑定，不能自动猜测。需要模型的步骤使用已注册路由；缺失授权、能力或版本绑定会明确拒绝。浏览器可选模型角色若填写，必须为 execute，且符合视觉 family 与能力要求。
4. 核对所列 planning permissions，明确确认后点击 **Prepare project plan**。此确认仅用于计划校验，不授予执行或系统权限。计划固定流程、项目绑定、原始契约断言、实参及模型注册修订/候选顺序；保留已校验的模型端点及能力和授权事实，不含密钥或标签。后续修改不会改变旧计划。
5. 在 **Saved project plan** 选择历史项，点击 **View saved plan** 查看冻结快照。计划历史限制为 100 条/8 MiB，达到上限会拒绝新写入并保留已有记录。**Server capability catalog** 显示规划支持与实际 runner 可用性；当前所有 runner 均为 false，计划 executable=false。

F03 已按治理与计划边界验收；v1 不自动迁移为执行定义。schema 2 操作如下：

1. 选择 **Execution project** 创建草稿，填写身份及标题。**Add wait step** 添加显式等待；依赖勾选框决定先后，Move 只允许保持合法依赖的调序。新增步骤默认依赖上一步，移除前先解除相关依赖/输出。
2. 使用 **Approved case to reuse** 选择已审核用例，再选 **Reviewed source contract** 和兼容视觉模型，点击 **Add reviewed case step**。完整来源、脚本与原断言保持固定；没有契约来源、修订失效、权限不足、运行时缺失或模型不兼容会明确拒绝准备。
3. **Advanced typed inputs, conditions and outputs** 编辑有限类型与来源 JSON；错误结构保留原文待改，不进入表单。服务端仍是执行校验依据。视觉用例只运行一次；等待最多五次，受步骤及总预算约束。
4. **Validate execution draft** 核对实际执行顺序，批准保存；对精确保存稿再次校验，批准 **Publish execution flow**。修订冲突保留草稿，重新读取会明确丢弃编辑稿。发布不可原地覆盖。
5. 选择发布版本，填实际输入，审核权限后准备。打开 **Review frozen preparation before running** 检查完整用例、契约、目标、模型端点及配置身份；准备十分钟有效且一次消费。配置变化要求重新准备，凭据仅留服务内存。再次明确批准 **Run prepared execution** 才入队，入队后改模型不改变已审任务。
6. **Execution history and cleanup** 显示步骤、尝试、失败原因及清理状态；刷新恢复事实，不重放。活动任务可 **Cancel execution**；超时/取消等受管资源退出和持久事实完成。每个已运行用例的 run ID 可在 Runs 中生成/查看官方 Allure。业务失败不会被清理成功覆盖。

服务重启将遗留执行标为 interrupted。若实际浏览器尝试的资源清理无法确认，后续动作保持隔离；重启不会解除，需核对恢复边界，不能通过删记录假装完成。纯等待中断无需浏览器隔离。schema 2 的实际 HTTP、核心和浏览器验收进度见[执行验收方案](plans/flow-execution-acceptance.md)。M04 自动视觉备用与真实项目视觉验收仍独立待办。

## 5. Models 与 Settings：配置和切换模型

无需模型密钥即可浏览已有资产。新的模型选择使用 **Models and routing**；**Settings** 仍提供基础模型兼容配置、CDP 端口和探索限制。

### 5.1 注册模型

在 Models 点击 **New profile**，填写 Profile ID、Label、**Protocol adapter**、Base URL、Model name 和 API key：

- **OpenAI compatible Chat Completions**：兼容 `/chat/completions` 的端点；声明所需 text/vision/json/tools 能力，视觉任务还需按已安装引擎支持范围填写 Midscene model family。
- **Anthropic Messages**：原生 `/messages` 协议，Base URL 使用 API 根目录（例如 `https://api.anthropic.com/v1`）；仅支持 text/json，用于生成和评审等文本操作，不用于当前 Midscene 视觉执行。版本固定 `2026-10-21`，不支持 temperature；使用 Messages 结构化输出参数，本地验证 JSON 语法，上游执行请求中的 schema，业务文档仍由领域层校验。

切换协议保留端点和模型名称，但清空输入密钥及不兼容的能力/family；请核对目标并明确重新填写密钥。改变已保存端点或协议时，服务端拒绝沿用旧密钥。协议错误、截断、拒绝或未知内容不能作为成功输出；限流/暂时错误只可使用预先批准的候选，不自动新增备用。完整证明范围见[协议验收](plans/model-protocol-acceptance.md)。

保存只校验配置，不代表已在线验证厂商能力。视觉路由需要同时声明 text 与 vision，并提供引擎支持的 Midscene family。视觉模型需要兼容实际 Midscene 版本；任意文本模型不能直接用于视觉任务。

对远程接收任务数据的模型，审阅并勾选 **Authorize this profile to process task data remotely, including when used as fallback**。备用模型也需要自己的数据授权；未授权或能力不匹配时应修正配置，不应绕过限制。

保存后 API key 输入清空，页面标记是否已保存。空值保存保留原密钥，输入新值替换，**Clear saved key** 明确删除。Settings 中主、备用密钥遵循相同保留/清除规则，普通读取不会回传原密钥。

### 5.2 选择任务路由

在 **Task routes** 选择 Task role、Primary profile，必要时填写按顺序排列的备用 Profile ID，点击 **Save task route**。

- 页面提供 `generate`、`explore`、`execute`、`correct` 角色配置；`correct` 用于上述受管证据的待审契约建议，不自动修复、发布或迁移。
- 当前生成链路支持有界备用链；探索和视觉执行只接受一个兼容主模型，页面禁用其自动备用输入。
- 一旦注册库包含配置，任务按注册库路由解析；不能只填写 Settings 就假定缺失的任务路由会自动回退到旧配置。请为实际使用的任务补齐路由。
- 已开始任务固定原选择；改变路由用于后续任务，不热改正在运行的模型，也不重写契约断言。

通过 **Route scope** 选择项目可覆盖全局路由，**Use global default** 删除所选覆盖。**Test saved profile** 仅向已保存端点发送合成文本，分别观测文本/JSON；未保存表单不参与检测。检测期间锁定编辑，结果注明实际已保存配置；选择其他配置清除旧结果。视觉/tools/Midscene 兼容不由此认证，视觉选项禁用 Messages 配置。schema 2 可逐步骤明确选择一个兼容视觉模型；自动视觉备用策略仍待实现。模型故障与业务断言失败应分开诊断，切换模型不能作为隐藏业务失败的手段。

### 5.3 预览并迁移已知配置

在 Settings 的 **Configuration migration** 中，可选 **Optional managed project override**，点击 **Find known configuration sources**。来源限定为当前配置、操作系统全局配置、旧数据目录配置及所选受管项目的配置覆盖；不能输入任意本机路径。选择可读来源后点击 **Preview selected migration**，比较脱敏的原目标、源和结果摘要。

通过 **Explicit credential choice** 明确选择 **Keep both target credentials (empty if no target exists)** 或 **Replace both credentials from source (empty source clears target)**。前者保留目标主/备用凭据，后者一并替换，源为空会清除目标。两种策略都使用来源的端点，必须核对这些端点之后再决定将来调用时可接收哪些凭据。环境变量引用保留为引用；此操作不导入模型注册库、项目资产或授予新的远程数据权限。

勾选结果确认，再点击 **Apply reviewed migration**。切换来源/项目或重新预览会清除策略和确认，改变凭据策略也会清除确认。预览有效期 10 分钟，新预览取代旧预览；源或目标的原文变化即使修订号相同也拒绝旧确认，需要重新预览。迁移仅在无活动任务时应用，写入前备份旧目标、保留源文件，页面未保存设置会被迁移结果替换。重复相同结果不增加修订。坏格式、未知字段或不安全链接不会静默丢弃/转换；先保留原文件并修正来源。

## 6. 单独准备 Playwright + Midscene.js 运行时

基础服务嵌入 Web 界面；实际视觉执行及官方 Allure HTML 生成仍需准备运行时。基础包不自动下载 Node 或执行引擎。在 **Environment** 可原生导入独立归档并维护默认运行时，无需用户安装 Python 或全局 Node；浏览器安装仍需单独明确授权。

### 6.1 导入、升级和回滚

选择 **Runtime archive (.tar.gz)**，填写可信渠道的 SHA-256，点击 **Upload and verify archive**。归档上限 1 GiB，分块为 512 KiB；校验来源、宿主和逐文件内容后才生成预览。核对版本、来源和摘要，勾选 **Approve this exact runtime change** 后确认安装。预览 10 分钟到期；刷新保留预览但清除批准，状态变化要求重新准备。使用 **Discard pending preview** 丢弃预览；失败或中断保留的上传可用 **Discard retained upload** 丢弃。

升级成功将原 current 保留为 previous；已有更旧备份时需明确允许成功后丢弃。点击 **Prepare rollback**，核对并批准 **Confirm rollback** 才切换。切换重新核验完整性并运行健康检查，失败恢复原安装。维护要求运行、排队和取消清理全部结束；维护清理完成前拒绝新任务。重启不自动重放安装，恢复失败时资产及已有报告仍可管理，引擎动作会拒绝执行。

点击 **Check available stable runtimes** 才会查询固定 `mbpz/qinAegis` GitHub 发布源；选择兼容版本并勾选下载授权，再点击 **Download and verify selected runtime**。下载后的预览仍需独立批准安装。未发布兼容的稳定归档及摘要时显示空列表；不提供任意 URL，不将 CI 候选视为稳定版本。

### 6.2 诊断与独立浏览器安装

Environment 使用服务解析出的固定运行时检查 Node、已安装包版本和真实无头 Chromium 启动，不接受任意命令或路径执行。**Install Chromium** 是显式联网安装操作，须先确认授权；它安装该运行时匹配的浏览器并重新检查，不自动安装操作系统依赖或修复整个运行时。检查/安装作为任务记录，可在 Tasks 查看状态；探测成功也不代表视觉模型或业务测试已经通过。

- 显式外部运行时覆盖：启动服务前设置 `QINAEGIS_RUNTIME_DIR` 为绝对目录，目录内需有有效 `runtime.json`、编译执行器、生产依赖、兼容 Node 和浏览器。只有 manifest 不足以运行。
- 开发运行时：准备仓库根 `sandbox` 的锁定依赖、执行器和浏览器，再设置 `QINAEGIS_DEV_SANDBOX` 为其绝对路径。不隐式回退到编译机器目录。

默认受管安装无需这两个变量。页面会显示当前选择；升级受管安装不会替换外部或开发覆盖。

完整清单、已核对的开发准备命令和系统依赖边界见 [INSTALL_zh.md](../INSTALL_zh.md#playwright--midscenejs-引擎)。环境变量修改后重启服务。性能和压测适配器还需要已安装且兼容的 Lighthouse / Locust 环境；缺工具、非零退出、超时或缺指标都明确失败，真实工具验收尚未完成。Lighthouse 不自动下载或上传公开报告。ZAP 在受管容器取消实现前明确不可用。

## 7. 探索、生成与人工审核

### Explore

输入经授权的目标 URL 和探索深度后启动任务，查看实际输出和终态。先准备视觉模型路由与引擎；不要将探索完成视为业务断言已通过。

### Generate

选择现有项目，填写需求。默认勾选 **Use published contracts bound to this project**；先在 Contracts 完成发布及绑定，再点击 **Refresh contract preview** 核对契约、输入和断言。预览不调用模型；正式生成会在服务端重新取得实际快照，因此预览后发生变更时应重新检查生成来源。

点击 **Generate Cases** 后，生成用例保持待审核状态。取消勾选使用契约时，明确按需求生成，不会附带当前项目契约断言。生成保存脚本及其来源；旧资产可能没有来源，不会被追认到当前契约。

### Review

选择项目并查看用例，点击 **Inspect source** 阅读完整 YAML 原文及断言、单独保存的 `expected_result`，并展开生成 provenance 检查契约输入和脱敏模型来源，再使用状态选择框审核。Reviewed / Approved 只针对本次载入的完整内容：服务比较原始资产的内容摘要（exact-content CAS），并发编辑或状态变更会拒绝旧审核。每次状态更新或刷新后，需要重新 Inspect 才能继续审核。

| 当前状态 | 允许转到 |
|---|---|
| Draft | Reviewed |
| Reviewed | Approved、Draft |
| Approved | Flaky、Archived |
| Flaky | Approved、Archived |
| Archived | 无后续状态 |

只有 Approved 用例可执行。来源缺失时按旧资产处理，不能把空来源当作审核通过证据。脚本执行通过也不独立证明生成脚本完整保留了业务语义。

需要纠正用例时点击 **Edit case**，对照原始完整脚本和预期结果，修改名称、完整 YAML 与 `expected_result`。勾选本次人工确认后点击 **Save as Draft**。保存会将状态重置为 Draft，清除不再适用于手工内容的生成 provenance，需要重新 Inspect、Reviewed、Approved；已冻结的历史运行证据保持不变。编辑同样使用载入时的内容摘要；冲突时表单保留，先另行保留待合并内容，再重新载入当前用例、比较并重新审核，不能重复提交旧摘要绕过检查。

## 8. 运行与任务停止

进入 **Run Tests**，选择项目和 smoke / functional / performance / stress 类型，点击 **Preview Plan** 核对实际计划、匹配用例与目标，再点击 **Run Tests**。没有匹配的已批准用例、引擎缺失或目标冲突应先解决，不能当作零失败通过。

运行前固定批准脚本、契约与输入、生成来源、脱敏模型及目标；后续改绑定或路由不会改写本次依据。终态保存独立执行记录，已有用例被更改时不能据当前文件替换历史证据。

同一服务使用一个执行槽，最多接受八个 FIFO 等待任务，包括探索、生成、执行、Allure 生成、模型探测与环境操作。Tasks 显示 queued、队列位置与等待原因；满额明确拒绝。提交时冻结输入，之后修改配置不改变已提交任务。排队任务可取消且不启动外部动作。业务结果已经产生但清理和最终历史保存尚未结束时，查询仍显示运行中；完成清理和落盘后才启动下一项，持久化失败会阻止后续动作。

任务进度中的 **Stop** 发出取消请求后会进入 `cancelling`，直到 worker 完成清理才显示 `stopped`。Unix 使用独立受管进程组，Windows 使用在恢复子进程执行前绑定的 Job Object，终态等待受管清理并传播清理错误。本批 Windows CI 已通过，逐引擎时限仍待验收，取消不是即时完成保证。服务会拒绝在活动线程期间正常退出。关闭标签页不等于停止；任务历史和有界日志会保存，重启将未结束任务标为 `interrupted`，不会自动重放或继续执行。已提交的业务终态不会被迟到的取消请求改写。

## 9. Reports：证据、门禁与 Allure

在 **Reports** 选择真实项目，再选择 Recent Runs 中的运行。首先核对运行终态、已执行数量、错误和质量门禁；部分脚本通过但运行失败、取消或清理失败时，门禁仍可能阻断，不能只看 passed 数量。

### 运行证据

展开 **Frozen inputs, sources and result evidence**，核对脚本、冻结输入、契约/生成来源、脱敏模型、已有结果、异常和截图。点击 **Download run evidence** 下载该次 JSON。旧摘要报告可能没有执行来源，也可能无法生成 Allure；程序不会补造历史。

### 官方 Allure 报告

1. 选有保存执行记录的运行，点击 **Generate Allure report**。
2. 等待独立报告任务完成；生成需要同一 Node 引擎及锁定的 `allure@3.33.0` 依赖，不调用模型，也不重跑测试。
3. 已存在报告时用 **View saved Allure report** 读取；页面显示报告器版本并在隔离区域展示。
4. 点击 **Download Allure HTML** 下载包含附件的单文件 HTML，分享前审查数据。

执行终态会导出标准 Allure 结果，HTML 按上述操作显式生成。报告生成失败独立显示错误，不改写 `run.json`、原执行结论或质量门禁。Allure 展示已有结果及生命周期异常，不凭空生成逐步动作或时间线；报告可以打开不等于测试通过。

页面还提供普通 HTML 查看和 **Export All Reports (JSON)** 项目报告导出。当前 Allure 按单次运行展示，尚无跨次趋势服务。performance/stress 无指标时显示缺失，不应解释为零分或通过。

### 报告生成中断后的维护

在已终态运行的 **Report recovery** 中点击 **Inspect report leftovers**。页面列出可清理的临时文件数量/大小，以及需要人工审查且会保留的项目。核对列表，勾选 **Remove only the temporary files listed in this preview**，再点击 **Clean inspected temporary files**。服务必须空闲；预览有效期 300 秒。重新检查、切换运行或一次清理尝试都会清除原确认；文件变化或预览过期时需重新检查。

只清理带有效本服务归属标记且内容核对通过的暂存目录或未完成报告。旧版无标记、未知文件、链接、损坏或超限残留保留供人工审查；不能通过递归删除整个 reports 目录解决。`run.json`、`allure-results` 和已有完整报告始终保留，维护不重跑测试、不改变门禁。

## 10. 常见失败与处理

| 现象 | 处理 |
|---|---|
| 无法连接、授权失效或 401 | 先 `qinaegis status`，再 `qinaegis open`；检查实际端口和会话，不分享启动 URL |
| 指定端口占用或已有实例端口不同 | 停止现有空闲实例后用新端口启动；显式端口不自动换号 |
| 库或模型 revision 冲突 | Refresh library / Refresh registry，比较新状态后重新审核提交；保留输入不代表旧确认仍有效 |
| 绑定/迁移参数被拒绝 | 检查名称是否完整且没有多余字段，逐项核对输入类型及目标版本是否弃用 |
| 模型未配置、能力或远程授权不满足 | 检查任务角色路由、Enabled、能力声明、Midscene family 与每个接收方授权；注册声明不等于真实兼容性 |
| 保存后密钥显示空 | 查看已保存标记；空输入会保留，显式清除才删除，不需要为查看明文而重新填写 |
| 引擎或 Allure 无法启动 | 核对运行时绝对路径、manifest、真实执行器、Node/锁定依赖和浏览器；环境改变后重启 |
| 没有可运行用例或目标冲突 | 核对项目、类型、Approved 状态、原脚本目标及生成来源；不要改断言绕过错误 |
| 任务一直 cancelling / 服务拒绝停止 | 等待线程清理并保留错误诊断；清理失败/超时应保留诊断，不启动重复任务争用资源 |
| 重启后任务 interrupted | 检查已有运行证据和外部副作用，再决定是否重新提交；服务不会自动重放任务 |
| 没有 HTML 或 Allure 按钮不可用 | 先看是否有保存执行记录；结构化证据不要求普通 report.html 存在，旧摘要也无法自动补齐来源 |
| 脚本都通过但门禁 Blocked | 检查运行/初始化/清理异常及结果覆盖，查看 Allure 独立生命周期异常；报告生成不会解除门禁 |

浏览器执行器独立启动 Chromium，不连接继承的 CDP 地址，各有私有工作目录。已实测双执行器 Cookie 隔离、关闭单项不影响另一项及已有外部浏览器。Unix 清理前复核目录身份；Windows 身份无法复核或受管进程未退出时保留目录并记录诊断，完整残留恢复仍待验收。

## 11. 当前能力与后续边界

当前开发分支已有本地 Web 生命周期与会话隔离、契约草稿/发布/纠正/弃用/派生/绑定迁移/受控交换、模型注册和任务角色路由、人工用例审核、冻结执行证据及官方 Allure 单次报告。这些代码和本地协议验收基础仍需真实业务及正式平台验收。

以下仍在后续范围，不能通过当前页面推定已经交付：

- schema 2 已支持等待/已审核用例执行、类型化数据流、有限条件、时限与清理；历史 v1 计划保持不可执行。自动视觉备用和更广泛能力组合仍待补齐，不能将两类计划混为一谈。
- 整体自动修复、完整稳定分发、操作系统依赖管理和跨平台干净机器验收；已有原生 Web 归档导入、显式稳定资产查询/下载、预览升级/回滚及单独授权的 Chromium 安装。
- 多资源并行调度、浏览器工作目录异常残留恢复及逐引擎取消时限验收；已有单槽有界队列、持久历史和重启中断标记，重启不继续执行；Windows Job Object 清理已通过真实 Windows CI。
- 视觉/tools/Midscene 在线能力验证、预批准步骤视觉降级与真实双项目视觉闭环；已有项目路由、步骤显式模型选择、两种独立文本协议和合成文本/JSON 探测。
- 完整 Trace/Console/Network 采集、逐步过程查看、证据保留/项目备份、统一变更审计与需求覆盖映射。当前只有实际截图/失败步骤/结构化结果及冻结来源，不能按“全量证据”理解。
- 可复核的模型失败辅助归因、暂停/继续/人工接管；现有 Stop 为取消，服务重启保留 interrupted，不恢复浏览器或重放动作。
- 操作系统无障碍电脑辅助。它是须明确授权的可选规划能力，安装或浏览器会话不授予电脑控制权限。
- Windows 用户级安装器的摘要/归档校验、运行保护、升级/回滚和保留数据卸载已通过真实 Windows CI；各系统干净机器、默认浏览器自动打开、签名及完整正式发布仍待验证。

CI 候选包、网站预览、源码版本号和单元测试通过均不能替代上述验收。更多分发状态见 [CI 与网站交付](CI_INTEGRATION.md)，后续顺序见[开发进度](progress/local-web-2.0.md)。

三维能力与当前边界见[能力复盘](plans/capability-map-review-2.0.md)。本地持久化和独立浏览器进程不是整个产品零运行依赖、绝不出网或操作系统安全沙箱；远程模型需数据授权。性能和压测需真实 Lighthouse/Locust 工具与指标，当前适配和 fixture 通过不代表真实工具已验收。复制项目目录也不等于完整一致性备份与跨平台迁移。

## 12. 配置恢复与任务历史

Settings 使用修订检查保存；过期标签保存失败会保留表单，重新读取当前配置后再比较修改。修改前保留私有备份；勾选恢复确认后可恢复上次配置，恢复同时保存现状并增加修订。损坏配置/备份不自动覆盖。

**Tasks** 显示最近最多 100 条任务和每条最多 64 KiB 的 UTF-8 日志尾部，截断有提示；日志按字节游标增量读取，刷新可继续查看已保存历史。活动任务日志会定期保存，故强制崩溃时最后一段尚未保存的输出可能丢失。服务异常退出的活动任务在下次启动时标为 **interrupted**，不会自动重放；需要自行审查外部副作用后决定是否重新提交。损坏历史会显式阻止启动，不自动覆盖；运行中的持久化失败也会显示错误。队列最多八项，任务摘要持久化但执行闭包不重放；排队任务也会在重启时变为 interrupted。

逐项验收和真实环境条件见 [验收 checklist](progress/acceptance-checklist-2.0.md)。


### 生成失败、取消和实际模型证据

在 **Task history** 选择生成任务，展开 **Inspect attempts and frozen model inputs**，可查看 generator/各例 critic 的实际候选与调用结果；使用提交时的模型配置，之后的路由修改不改变历史。限流、服务不可用、传输故障、超时及拒绝分别显示。业务生成文档无效时，即使收到模型响应，任务仍失败；critic 未成功的草稿明确显示未验证，需要人工审核。

started 仅表示本地请求准入已持久化，不证明远端已接收或完成。取消保留候选事实，服务强杀后保留 interrupted 与最后已知意图，均不自动重放。较早已保存草稿可能继续存在，应逐例检查；取消不能撤销远端已发生的请求或已提交的原子文件写入。服务在每次模型阶段与保存前检查取消。

模型回显本任务已配置凭据时，生成服务明确拒绝并保留安全错误，不把凭据写入生成用例或评审，不替换脚本文本。未登记的秘密不由此自动识别。旧配置路由显示 actual attempts not recorded，应使用注册模型任务路由获取准确逐候选事实。范围和复现见[生成证据验收](plans/generation-evidence-acceptance.md)。
