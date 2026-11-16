# QinAegis 2.0 用户指南

更新：2026-11-16。适用于当前 2.0 开发分支，**不代表完整 2.0 已正式发布**。QinAegis 的核心资产是可复用测试契约；本地 Web 工作台负责维护契约、项目绑定、模型路由、审核、执行和报告。视觉测试基于 **Playwright + Midscene.js**。

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

推荐先建立和审核契约，再绑定项目、生成并审核用例，最后执行和查看报告。也可直接浏览已有资产，或在 Generate 中明确选择不使用契约；探索不是每次执行都必须重跑的前置操作。当前尚无可运行的任意流程编排器。

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

## 5. Models 与 Settings：配置和切换模型

无需模型密钥即可浏览已有资产。新的模型选择使用 **Models and routing**；**Settings** 仍提供基础模型兼容配置、CDP 端口和探索限制。

### 5.1 注册模型

在 Models 点击 **New profile**，填写 Profile ID、Label、Base URL、Model name 和 API key；当前内置适配器为兼容 OpenAI Chat Completions 的端点。声明 text/vision/json/tools 能力，并按已安装引擎支持范围填写 Midscene model family。

保存只校验配置，不代表已在线验证厂商能力。视觉路由需要同时声明 text 与 vision，并提供引擎支持的 Midscene family。视觉模型需要兼容实际 Midscene 版本；任意文本模型不能直接用于视觉任务。

对远程接收任务数据的模型，审阅并勾选 **Authorize this profile to process task data remotely, including when used as fallback**。备用模型也需要自己的数据授权；未授权或能力不匹配时应修正配置，不应绕过限制。

保存后 API key 输入清空，页面标记是否已保存。空值保存保留原密钥，输入新值替换，**Clear saved key** 明确删除。Settings 中主、备用密钥遵循相同保留/清除规则，普通读取不会回传原密钥。

### 5.2 选择任务路由

在 **Task routes** 选择 Task role、Primary profile，必要时填写按顺序排列的备用 Profile ID，点击 **Save task route**。

- 页面提供 `generate`、`explore`、`execute`、`correct` 角色配置；配置角色不代表对应自动纠正流程已经实现。
- 当前生成链路支持有界备用链；探索和视觉执行只接受一个兼容主模型，页面禁用其自动备用输入。
- 一旦注册库包含配置，任务按注册库路由解析；不能只填写 Settings 就假定缺失的任务路由会自动回退到旧配置。请为实际使用的任务补齐路由。
- 已开始任务固定原选择；改变路由用于后续任务，不热改正在运行的模型，也不重写契约断言。

当前尚无在线能力检测、项目/步骤级策略或任意协议适配能力。模型故障与业务断言失败应分开诊断，切换模型不能作为隐藏业务失败的手段。

## 6. 单独准备 Playwright + Midscene.js 运行时

基础服务嵌入 Web 界面；实际视觉执行及官方 Allure HTML 生成仍需准备运行时。基础包暂不自动下载 Node、执行引擎或浏览器。

- 安装式运行时：启动服务前设置 `QINAEGIS_RUNTIME_DIR` 为绝对目录，目录内需有有效 `runtime.json`、编译执行器、生产依赖、兼容 Node 和浏览器。只有 manifest 不足以运行。
- 开发运行时：准备仓库根 `sandbox` 的锁定依赖、执行器和浏览器，再设置 `QINAEGIS_DEV_SANDBOX` 为其绝对路径。不隐式回退到编译机器目录。

完整清单、已核对的开发准备命令和系统依赖边界见 [INSTALL_zh.md](../INSTALL_zh.md#playwright--midscenejs-引擎)。环境变量修改后重启服务。性能和压测适配器还需要各自兼容的 Lighthouse / Locust 环境，不因基础服务可启动而自动可用。

## 7. 探索、生成与人工审核

### Explore

输入经授权的目标 URL 和探索深度后启动任务，查看实际输出和终态。先准备视觉模型路由与引擎；不要将探索完成视为业务断言已通过。

### Generate

选择现有项目，填写需求。默认勾选 **Use published contracts bound to this project**；先在 Contracts 完成发布及绑定，再点击 **Refresh contract preview** 核对契约、输入和断言。预览不调用模型；正式生成会在服务端重新取得实际快照，因此预览后发生变更时应重新检查生成来源。

点击 **Generate Cases** 后，生成用例保持待审核状态。取消勾选使用契约时，明确按需求生成，不会附带当前项目契约断言。生成保存脚本及其来源；旧资产可能没有来源，不会被追认到当前契约。

### Review

选择项目并查看用例，点击 **Inspect source** 阅读原脚本、契约输入和脱敏模型来源，再使用状态选择框审核：

| 当前状态 | 允许转到 |
|---|---|
| Draft | Reviewed |
| Reviewed | Approved、Draft |
| Approved | Flaky、Archived |
| Flaky | Approved、Archived |
| Archived | 无后续状态 |

只有 Approved 用例可执行。当前页面提供来源检查和状态审核，不提供完整的可视化脚本编辑器。来源缺失时按旧资产处理，不能把空来源当作审核通过证据。脚本执行通过也不独立证明生成脚本完整保留了业务语义。

## 8. 运行与任务停止

进入 **Run Tests**，选择项目和 smoke / functional / performance / stress 类型，点击 **Preview Plan** 核对实际计划、匹配用例与目标，再点击 **Run Tests**。没有匹配的已批准用例、引擎缺失或目标冲突应先解决，不能当作零失败通过。

运行前固定批准脚本、契约与输入、生成来源、脱敏模型及目标；后续改绑定或路由不会改写本次依据。终态保存独立执行记录，已有用例被更改时不能据当前文件替换历史证据。

当前保守限制同一服务同时一个工作线程，包括探索、生成、执行与 Allure 生成。出现 Another task is running or cancelling 时等待前一线程真实退出。

任务进度中的 **Stop** 发出取消请求后会进入 `cancelling`，直到 worker 完成清理才显示 `stopped`。长引擎调用目前可能无法立即中断，服务也会拒绝在活动线程期间正常退出。关闭标签页不等于停止；当前任务状态在内存中，不承诺服务重启后恢复队列或日志。

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
| 任务一直 cancelling / 服务拒绝停止 | 等待线程清理并保留错误诊断；当前不承诺立即中断与崩溃恢复，不启动重复任务争用资源 |
| 没有 HTML 或 Allure 按钮不可用 | 先看是否有保存执行记录；结构化证据不要求普通 report.html 存在，旧摘要也无法自动补齐来源 |
| 脚本都通过但门禁 Blocked | 检查运行/初始化/清理异常及结果覆盖，查看 Allure 独立生命周期异常；报告生成不会解除门禁 |

## 11. 当前能力与后续边界

当前开发分支已有本地 Web 生命周期与会话隔离、契约草稿/发布/纠正/弃用/派生/绑定迁移/受控交换、模型注册和任务角色路由、人工用例审核、冻结执行证据及官方 Allure 单次报告。这些代码和本地协议验收基础仍需真实业务及正式平台验收。

以下仍在后续范围，不能通过当前页面推定已经交付：

- 完整运行编排器、类型化数据流、条件分支与清理策略；目前只有场景依赖校验和稳定拓扑排序基础。
- Node/引擎/浏览器自动安装、修复和完整 Web 环境管理。
- 持久任务队列、增量日志、可靠强取消、子进程树回收与重启恢复。
- 在线模型能力检测、项目/步骤级路由、更多协议适配与真实双项目视觉闭环。
- 操作系统无障碍电脑辅助。它是须明确授权的可选规划能力，安装或浏览器会话不授予电脑控制权限。
- Windows 一键安装、各系统干净机器和签名验收、更新/回滚与完整正式发布。

CI 候选包、网站预览、源码版本号和单元测试通过均不能替代上述验收。更多分发状态见 [CI 与网站交付](CI_INTEGRATION.md)，后续顺序见[开发进度](progress/local-web-2.0.md)。
