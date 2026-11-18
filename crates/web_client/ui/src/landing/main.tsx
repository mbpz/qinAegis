import { useState } from 'react';
import { createRoot } from 'react-dom/client';
import { ArrowRight, Check, Copy, GitBranch, Layers, ShieldCheck } from 'lucide-react';
import { Button, ButtonLink } from '@/components/motion/button/base';
import { Collapsible, CollapsibleContent, CollapsibleIndicator, CollapsibleTrigger } from '@/components/motion/collapsible';
import './pages.css';

declare const __SOURCE_REF__: string;
const repo = 'https://github.com/mbpz/qinAegis';
const sourceRef = __SOURCE_REF__;
const source = `${repo}/tree/${encodeURIComponent(sourceRef)}`;
const doc = (name: string) => `${repo}/blob/${encodeURIComponent(sourceRef)}/${name}`;
const commands = `git clone --branch ${sourceRef} https://github.com/mbpz/qinAegis.git\ncd qinAegis/crates/web_client/ui\nnpm ci\nnpm run build\ncd ../../..\ncargo build --locked --release --bin qinaegis\n./target/release/qinaegis`;
const faq = [
  ['2.0 现在可以直接下载正式安装包吗？', '2.0 尚未正式发布。目前提供源码构建路径，CI 正在为独立本地服务生成候选包。仓库历史 v1.0 发布于 2026 年 11 月 13 日，其桌面包不代表 2.0。正式安装包需要通过对应平台验收后发布。'],
  ['安装本地服务后就能直接执行视觉测试吗？', '还需要准备 Playwright + Midscene.js 引擎、兼容的 Node 与自动化浏览器。基础服务嵌入 Web 界面，浏览已有资产无需模型密钥；执行视觉测试需要准备运行时，在设置中配置模型。原生 Web 归档导入、预览升级/回滚及稳定来源查询入口已接入，完整稳定分发和干净机器仍待验收。'],
  ['契约怎样复用、维护和纠正？', '把业务意图、输入、前置条件和断言留在契约，把地址、定位与环境差异留在项目绑定。契约支持版本、修订与发布流程；执行保留版本快照和证据，纠正产生新的修订，避免覆盖历史执行依据。具体工作流与验收范围见实施计划。'],
  ['数据是否始终只留在本机？', '服务监听本机回环地址，项目资产保存在本机。使用远程模型时，测试页面内容、截图或提示可能发送给所选模型服务。请按项目数据策略选择模型路由，避免在共享契约包中包含密钥与私有数据。'],
  ['会自动获得控制电脑的权限吗？', '不会。基于操作系统无障碍授权的电脑辅助属于规划能力，需要用户明确授权，并受权限边界和审计约束；当前页面不代表该能力已经交付。'],
];

function App() {
  const [copyState, setCopyState] = useState('复制命令');
  async function copyCommands() {
    try { await navigator.clipboard.writeText(commands); setCopyState('已复制'); }
    catch { setCopyState('请选中下方命令复制'); }
  }
  return <>
    <a className="skip-link" href="#main">跳到主要内容</a>
    <header className="site-header"><a className="brand" href="#" aria-label="QinAegis 首页"><span className="brand-mark" aria-hidden="true">Q</span>QinAegis<span className="brand-version">2.0</span></a><nav aria-label="主要导航"><a href="#contract">契约核心</a><a href="#workflow">运行方式</a><a href="#start">开始使用</a><ButtonLink href={source} variant="outline" size="sm">GitHub <ArrowRight size={14} aria-hidden="true" /></ButtonLink></nav></header>
    <main id="main">
      <section className="hero wrap">
        <div className="hero-copy"><div className="eyebrow"><span className="status-dot" /> 2.0 开发中 · 本地 Web 工作台</div><h1>让测试契约，<br />跨项目<span>生长。</span></h1><p className="hero-intro">把可复用的业务意图留下来。<br />把项目差异、执行引擎和模型，交给各自的绑定。</p><p className="hero-detail">基于 Playwright + Midscene.js 的视觉驱动测试。启动本地服务，自动打开浏览器，在 Web 中完成配置与操作，无需桌面客户端打包。</p><div className="hero-actions"><ButtonLink href="#start" size="lg">构建 2.0 开发版 <ArrowRight size={18} aria-hidden="true" /></ButtonLink><ButtonLink href={doc('docs/plans/local-web.md')} variant="outline" size="lg">查看实施计划</ButtonLink></div><div className="hero-note">尚无 2.0 正式发布包 · 视觉执行需单独准备引擎与浏览器</div></div>
        <div className="contract-board" aria-label="同一契约复用到两个项目的示意图"><div className="board-toolbar"><span><span className="status-dot" /> CONTRACT WORKSPACE</span><span>结构示意 · 非运行结果</span></div><div className="contract-file"><div className="file-heading"><Layers size={18} aria-hidden="true" /><span>登录成功契约</span><span className="revision">rev. 03</span></div><dl><div><dt>意图</dt><dd>已注册用户能够进入工作台</dd></div><div><dt>输入</dt><dd>账号、密码与环境参数</dd></div><div><dt>断言</dt><dd>工作台可见，用户身份一致</dd></div><div><dt>证据</dt><dd>步骤、截图、断言与执行快照</dd></div></dl></div><div className="binding-label"><span />同一份契约 · 不同项目绑定<span /></div><div className="project-bindings"><div><small>PROJECT A</small><strong>商城后台</strong><code>登录页 / 管理员身份</code><span>绑定页面与数据</span></div><div><small>PROJECT B</small><strong>供应商平台</strong><code>门户入口 / 供应商身份</code><span>复用意图与断言</span></div></div><div className="board-footer"><GitBranch size={14} aria-hidden="true" /> 修改绑定，无需复制整套业务契约</div></div>
      </section>
      <div className="foundation-strip"><div className="wrap"><span>契约是资产</span><i aria-hidden="true" />项目是绑定<i aria-hidden="true" />模型可切换<i aria-hidden="true" />执行有证据</div></div>
      <section id="contract" className="section wrap"><div className="section-heading"><div className="eyebrow">01 / PRODUCT CORE</div><h2>积累的是契约，<br />不只是一次跑通的脚本。</h2><p>将稳定的业务语义与变化的技术细节分开，让自动化项目可以复用、维护，也可以纠正。</p></div><div className="principles"><article><span className="number">01</span><h3>复用业务意图</h3><p>契约描述目标、输入、前置条件与断言。项目绑定承接环境、页面与数据差异，减少跨项目复制。</p></article><article><span className="number">02</span><h3>修订有来处</h3><p>用修订、发布与快照维护演进依据。发现错误后纠正契约，保留原有执行记录，便于追溯影响。</p></article><article><span className="number">03</span><h3>组合真实场景</h3><p>场景、模型路由与执行器各司其职。schema 2 已支持等待与审核用例组合、合法调序和冻结执行；自动视觉备用及更广泛能力仍待补齐。</p></article></div></section>
      <section id="workflow" className="workflow-section"><div className="section wrap"><div className="section-heading"><div className="eyebrow">02 / LOCAL WEB, MODULAR EXECUTION</div><h2>一个浏览器入口，<br />一条可追溯的执行链路。</h2><p>本地服务承载工作台。测试执行、模型能力和报告各自解耦，让场景可以按需组合。</p></div><div className="workflow"><article><span>01 / 启动</span><h3>运行本地服务</h3><p>启动 qinaegis，自动打开默认浏览器。Web 资产随服务提供，无需桌面客户端。</p></article><ArrowRight className="flow-arrow" aria-hidden="true" /><article><span>02 / 配置</span><h3>绑定契约与能力</h3><p>选择项目、维护契约和模型路由，组合已审核用例与等待步骤；运行前核对冻结输入。</p></article><ArrowRight className="flow-arrow" aria-hidden="true" /><article><span>03 / 执行</span><h3>视觉驱动测试</h3><p>由独立准备的 Playwright + Midscene.js 运行时执行，保留实际返回的结果与证据。</p></article><ArrowRight className="flow-arrow" aria-hidden="true" /><article><span>04 / 复盘</span><h3>查看 Allure 报告</h3><p>结合运行状态、断言、附件与历史快照，定位问题并发起下一次纠正。</p></article></div><p className="execution-note"><ShieldCheck size={18} aria-hidden="true" />本机保存项目资产；远程模型可能接收页面内容或截图。请按数据策略配置模型。</p></div></section>
      <section className="section wrap delivery"><div className="section-heading"><div className="eyebrow">03 / DELIVERY BOUNDARY</div><h2>把方向与交付，<br />说清楚。</h2><p>2.0 正在迭代。以下为开发分支能力与后续边界，最终可用性以验收和正式发布说明为准。</p><ButtonLink href={doc('docs/plans/local-web.md')} variant="outline">阅读验收计划 <ArrowRight size={16} aria-hidden="true" /></ButtonLink></div><div className="delivery-list"><article><span className="tag">开发分支已接入</span><h3>本地 Web 与服务生命周期</h3><p>浏览器工作台、配置、服务启停和状态管理；执行保留证据与记录。</p></article><article><span className="tag">开发分支持续验收</span><h3>契约演进、模型路由与 Allure</h3><p>契约修订和项目绑定、可扩展模型接入与切换、视觉执行和报告链路，仍需真实环境回归。</p></article><article><span className="tag planned">后续规划</span><h3>完整过程证据与人机协同</h3><p>当前保留截图、结果与冻结来源；全量 Trace/Console/Network、辅助归因、暂停/继续/接管尚待实现。无障碍电脑控制仍需单独授权与试点。</p></article></div></section>
      <section id="start" className="start-section"><div className="section wrap start-grid"><div><div className="eyebrow">04 / BUILD THE DEVELOPMENT VERSION</div><h2>先在本机，<br />启动工作台。</h2><p>当前使用 2.0 开发源码构建。需要 Git、Node / npm，以及满足仓库要求的 Rust 工具链。下面命令构建并运行服务，首次启动会打开浏览器。</p><p>执行视觉测试还需准备 sandbox 依赖和浏览器，并显式设置 <code>QINAEGIS_DEV_SANDBOX</code>；安装方式、运行时配置与限制请阅读安装说明。</p><div className="start-links"><ButtonLink href={doc('INSTALL_zh.md')} variant="outline">安装与运行时说明 <ArrowRight size={16} aria-hidden="true" /></ButtonLink><a href={`${repo}/releases`}>查看发布记录（2.0 尚未发布）</a></div></div><div className="terminal"><div className="terminal-heading"><span>SOURCE BUILD / 2.0</span><Button variant="ghost" size="sm" className="copy-button" onClick={copyCommands}>{copyState === '已复制' ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}<span aria-live="polite">{copyState}</span></Button></div><pre tabIndex={0} aria-label="构建并运行开发版的命令"><code>{commands}</code></pre><div className="terminal-foot">源码分支：<code>{sourceRef}</code><br />关闭浏览器不会停止服务；请使用 qinaegis stop。</div></div></div></section>
      <section className="section wrap faq-section"><div className="section-heading"><div className="eyebrow">05 / BEFORE YOU START</div><h2>使用前，了解这些。</h2></div><div className="faq-list">{faq.map(([question, answer]) => <Collapsible key={question} className="faq-item"><CollapsibleTrigger className="faq-trigger">{question}<CollapsibleIndicator /></CollapsibleTrigger><CollapsibleContent contentClassName="faq-answer"><p>{answer}</p></CollapsibleContent></Collapsible>)}</div></section>
      <section className="closing wrap"><div><span className="eyebrow">BUILD ON A REUSABLE FOUNDATION</span><h2>让下一次测试，<br />站在已有契约之上。</h2></div><ButtonLink href={source} size="lg">查看 2.0 源码 <ArrowRight size={18} aria-hidden="true" /></ButtonLink></section>
    </main><footer className="wrap site-footer"><a className="brand" href="#">QinAegis <span className="brand-version">2.0 DEV</span></a><p>可复用契约 · 本地 Web 质量工作台</p><div><a href={doc('CONTEXT.md')}>产品记忆</a><a href={doc('docs/plans/capability-map-review-2.0.md')}>能力与边界</a><a href={`${repo}/issues`}>反馈问题</a><a href={doc('LICENSE')}>许可证</a></div></footer>
  </>;
}
createRoot(document.getElementById('root')!).render(<App />);
