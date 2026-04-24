import { chromium, Browser, Page } from 'playwright';
import { PlaywrightAgent } from '@midscene/web/playwright';

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

export async function extractPageInfo(page: Page): Promise<PageInfo> {
  const agent = new PlaywrightAgent(page);

  const info = await agent.aiQuery<{
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
    `分析当前页面，提取：标题、顶部导航、主要功能、是否需要登录、检测到的技术栈、表单信息、关键元素、所有内部链接`
  );

  return {
    url: page.url(),
    title: info.title,
    primaryNav: info.primaryNav,
    mainFeatures: info.mainFeatures,
    authRequired: info.authRequired,
    techStack: info.techStack,
    forms: info.forms,
    keyElements: info.keyElements,
    links: info.links.filter(l => !l.startsWith('http') || l.includes(new URL(page.url()).host)),
  };
}

export async function exploreProject(seedUrls: string[], maxDepth: number): Promise<PageInfo[]> {
  const visited = new Set<string>();
  const results: PageInfo[] = [];
  const queue: { url: string; depth: number }[] = seedUrls.map(u => ({ url: u, depth: 0 }));

  const browser = await chromium.connectOverCDP(process.env.CDP_WS_URL || 'ws://localhost:9222');

  while (queue.length > 0) {
    const { url, depth } = queue.shift()!;
    if (visited.has(url) || depth > maxDepth) continue;
    visited.add(url);

    const page = await browser.newPage();
    try {
      await page.goto(url, { timeout: 30000 });
      const info = await extractPageInfo(page);
      results.push(info);

      for (const link of info.links.slice(0, 10)) {
        const absUrl = new URL(link, url).href;
        if (!visited.has(absUrl)) {
          queue.push({ url: absUrl, depth: depth + 1 });
        }
      }
    } catch (e) {
      console.error(`Failed to explore ${url}: ${e}`);
    } finally {
      await page.close();
    }
  }

  await browser.close();
  return results;
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