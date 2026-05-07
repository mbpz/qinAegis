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

  // Launch a dedicated browser for exploration
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

  // Create isolated context for exploration
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
      await page.goto(url, { timeout: 30000 });
      console.error(`[explorer] Navigated to ${url}`);

      let info;
      try {
        info = await extractPageInfo(page);
        results.push(info);

        // Find links on the page
        for (const link of info.links.slice(0, 10)) {
          const absUrl = new URL(link, url).href;
          if (!visited.has(absUrl)) {
            queue.push({ url: absUrl, depth: depth + 1 });
          }
        }
      } catch (e) {
        console.error(`[explorer] AI extraction failed for ${url}: ${e}`);
        // Continue without this page's links
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

  const rawInfo = await agent.aiQuery<{
    title: string;
    primaryNav: string[];
    mainFeatures: string[];
    authRequired: boolean;
    techStack: string[];
    forms: { action: string; method: string; fields: string[] }[];
    keyElements: string[];
    links: string[];
  }>(
    `{title: string, primaryNav: string[], mainFeatures: string[], authRequired: boolean, techStack: string[], forms: {action: string, method: string, fields: string[]}[], keyElements: string[], links: string[]}, ` +
    `分析当前页面，提取：标题、顶部导航、主要功能、是否需要登录、检测到的技术栈、表单信息、关键元素、所有内部链接`,
    { screenshotIncluded: false }
  );

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

  return {
    url: page.url(),
    title: info.title || '',
    primaryNav: Array.isArray(info.primaryNav) ? info.primaryNav : [],
    mainFeatures: Array.isArray(info.mainFeatures) ? info.mainFeatures : [],
    authRequired: Boolean(info.authRequired),
    techStack: Array.isArray(info.techStack) ? info.techStack : [],
    forms: Array.isArray(info.forms) ? info.forms : [],
    keyElements: Array.isArray(info.keyElements) ? info.keyElements : [],
    links: Array.isArray(info.links) ? info.links.filter(l => !l.startsWith('http') || l.includes(new URL(page.url()).host)) : [],
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