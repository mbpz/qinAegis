import { chromium, Page, BrowserContext } from 'playwright';

const CDP_PORT = parseInt(process.env.CDP_PORT || '9222', 10);

export interface PageInfo {
  url: string;
  title: string;
  primaryNav: string[];
  mainFeatures: string[];
  authRequired: boolean;
  techStack: string[];
  forms: FormInfo[];
  keyElements: string[];
  links: string[];
}

export interface FormInfo {
  action: string;
  method: string;
  fields: string[];
}

export async function exploreProject(seedUrls: string[], maxDepth: number): Promise<PageInfo[]> {
  const visited = new Set<string>();
  const results: PageInfo[] = [];
  const queue: { url: string; depth: number }[] = seedUrls.map(u => ({ url: u, depth: 0 }));

  console.error(`[explorer] Launching Chromium for exploration on port ${CDP_PORT}...`);

  const browser = await chromium.launch({
    headless: true,
    args: [
      `--remote-debugging-port=${CDP_PORT}`,
      '--no-first-run',
      '--no-default-browser-check',
      '--disable-extensions',
      '--disable-popup-blocking',
      '--disable-translate',
      '--disable-gpu',
      '--disable-dev-shm-usage',
    ],
  });

  const context = await browser.newContext({
    ignoreHTTPSErrors: true,
  });

  let page: Page;
  try {
    page = await context.newPage();
    console.error(`[explorer] Created new page`);
  } catch (e) {
    throw new Error(`Failed to create new page: ${e}`);
  }

  while (queue.length > 0) {
    const { url, depth } = queue.shift()!;
    if (visited.has(url) || depth > maxDepth) continue;
    visited.add(url);

    try {
      console.error(`[explorer] Navigating to ${url}...`);
      await page.goto(url, { timeout: 30000, waitUntil: 'networkidle' });
      console.error(`[explorer] Navigated to ${url}`);

      console.error(`[explorer] Extracting page info with AI...`);
      let info;
      try {
        info = await extractPageInfo(page);
        console.error(`[explorer] AI extraction complete for ${url}`);
        results.push(info);

        for (const link of info.links.slice(0, 10)) {
          const absUrl = new URL(link, url).href;
          if (!visited.has(absUrl)) {
            queue.push({ url: absUrl, depth: depth + 1 });
          }
        }
      } catch (e) {
        console.error(`[explorer] AI extraction failed for ${url}: ${e}`);
      }
    } catch (e) {
      console.error(`[explorer] Navigation failed for ${url}: ${e}`);
    }
  }

  await browser.close();
  return results;
}

async function extractPageInfo(page: Page): Promise<PageInfo> {
  const { PlaywrightAgent } = await import('@midscene/web/playwright');
  const agent = new PlaywrightAgent(page);

  const pageUrl = page.url();
  const pageTitle = await page.title();
  const html = await page.content();
  console.error(`[explorer] pageUrl=${pageUrl}, title=${pageTitle}, html_len=${html.length}`);

  // Force screenshot first to verify page loaded
  const screenshot = await page.screenshot({ encoding: 'base64' as any });
  console.error(`[explorer] screenshot size=${screenshot.length} chars`);

  const prompt = `分析当前页面的截图和DOM。只描述你实际看到的内容：
- 标题: 页面的实际标题
- 导航: 实际存在的顶部导航项（没有就不填）
- 主要功能: 实际看到的功能模块
- 是否需要登录: 看到登录框=true，没登录框=false
- 技术栈: 实际能推断出的前端技术
- 表单: 实际存在的表单（如登录表单的账号/密码字段）
- 关键元素: 实际可见的关键UI元素
- 链接: 实际存在的内部链接（只填同类域名的）

JSON格式返回，字段不能为空数组就用[]：
{title, primaryNav, mainFeatures, authRequired, techStack, forms, keyElements, links}`;

  const rawInfo = await agent.aiQuery<{
    title: string;
    primaryNav: string[];
    mainFeatures: string[];
    authRequired: boolean;
    techStack: string[];
    forms: { action: string; method: string; fields: string[] }[];
    keyElements: string[];
    links: string[];
  }>(prompt, { screenshotIncluded: true });

  const info = rawInfo as {
    title?: string;
    primaryNav?: string[];
    mainFeatures?: string[];
    authRequired?: boolean;
    techStack?: string[];
    forms?: { action: string; method: string; fields: string[] }[];
    keyElements?: string[];
    links?: string[];
  };

  console.error(`[explorer] AI response: title=${info.title}, authRequired=${info.authRequired}, links_count=${info.links?.length}`);

  return {
    url: pageUrl,
    title: info.title || pageTitle || '',
    primaryNav: Array.isArray(info.primaryNav) ? info.primaryNav : [],
    mainFeatures: Array.isArray(info.mainFeatures) ? info.mainFeatures : [],
    authRequired: Boolean(info.authRequired),
    techStack: Array.isArray(info.techStack) ? info.techStack : [],
    forms: Array.isArray(info.forms) ? info.forms : [],
    keyElements: Array.isArray(info.keyElements) ? info.keyElements : [],
    links: Array.isArray(info.links) ? info.links.filter(l => !l.startsWith('http') || l.includes(new URL(pageUrl).host)) : [],
  };
}

export function toMarkdown(pages: PageInfo[]): string {
  let md = '# 项目规格书\n\n';
  for (const page of pages) {
    md += `## ${page.url}\n`;
    md += `- **标题**: ${page.title}\n`;
    md += `- **导航**: [${page.primaryNav.join(', ')}]\n`;
    md += `- **功能**: ${page.mainFeatures.join(', ')}\n`;
    md += `- **认证**: ${page.authRequired ? '需要登录' : '无需登录'}\n`;
    md += `- **技术栈**: ${page.techStack.join(', ')}\n`;
    if (page.forms.length > 0) {
      md += `- **表单**: ${page.forms.map(f => `${f.method.toUpperCase()} ${f.action} (${f.fields.join(', ')})`).join('; ')}\n`;
    }
    md += '\n';
  }
  return md;
}