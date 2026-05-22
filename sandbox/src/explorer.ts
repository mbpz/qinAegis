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

interface RawPageInfo {
  title?: unknown;
  primaryNav?: unknown;
  mainFeatures?: unknown;
  authRequired?: unknown;
  techStack?: unknown;
  forms?: unknown;
  keyElements?: unknown;
  links?: unknown;
}

function isString(v: unknown): v is string {
  return typeof v === 'string';
}

function isBool(v: unknown): v is boolean {
  return typeof v === 'boolean';
}

function isStringArray(v: unknown): v is string[] {
  return Array.isArray(v) && v.every(isString);
}

function isFormInfoArray(v: unknown): v is FormInfo[] {
  return Array.isArray(v) && v.every(item => {
    if (typeof item !== 'object' || item === null) return false;
    const obj = item as Record<string, unknown>;
    return isString(obj.action) && isString(obj.method) && isStringArray(obj.fields);
  });
}

function isValidLink(v: unknown, host: string): boolean {
  if (!isString(v)) return false;
  if (!v.startsWith('http')) return true;
  return v.includes(host);
}

function validatePageInfo(raw: RawPageInfo, pageUrl: string, pageTitle: string): PageInfo {
  const host = (() => {
    try { return new URL(pageUrl).host; } catch { return ''; }
  })();

  const links = raw.links;
  const validatedLinks: string[] = [];
  if (isStringArray(links)) {
    for (const l of links) {
      if (isValidLink(l, host)) validatedLinks.push(l);
    }
  }

  return {
    url: pageUrl,
    title: isString(raw.title) ? raw.title : (pageTitle || ''),
    primaryNav: isStringArray(raw.primaryNav) ? raw.primaryNav : [],
    mainFeatures: isStringArray(raw.mainFeatures) ? raw.mainFeatures : [],
    authRequired: isBool(raw.authRequired) ? raw.authRequired : false,
    techStack: isStringArray(raw.techStack) ? raw.techStack : [],
    forms: isFormInfoArray(raw.forms) ? raw.forms : [],
    keyElements: isStringArray(raw.keyElements) ? raw.keyElements : [],
    links: validatedLinks,
  };
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
      const response = await page.goto(url, { timeout: 30000, waitUntil: 'domcontentloaded' });
      console.error(`[explorer] HTTP status: ${response?.status()}, URL: ${page.url()}`);

      // Wait for network to settle AND page to be stable
      try {
        await page.waitForLoadState('networkidle', { timeout: 15000 });
        console.error(`[explorer] Network idle reached`);
      } catch (e) {
        console.error(`[explorer] Network idle timeout (continuing anyway): ${e}`);
      }

      // Additional wait for any JS to finish rendering
      await page.waitForTimeout(2000);

      const pageTitle = await page.title();
      const pageContent = await page.content();
      console.error(`[explorer] After-wait: title="${pageTitle}", html_len=${pageContent.length}`);

      // Verify page actually loaded something meaningful
      const bodyText = await page.evaluate(() => document.body?.innerText?.slice(0, 200) || 'EMPTY');
      console.error(`[explorer] Body text preview: "${bodyText.replace(/\n/g, ' ').slice(0, 100)}"`);

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
  const pageUrl = page.url();
  const pageTitle = await page.title();

  // Verify screenshot actually works and print size
  let screenshotSize = 0;
  try {
    const screenshot = await page.screenshot({ encoding: 'base64' as any, type: 'jpeg', quality: 80 });
    screenshotSize = screenshot.length;
    console.error(`[explorer] Verified screenshot: ${screenshotSize} chars, URL=${pageUrl}`);
  } catch (e) {
    console.error(`[explorer] Screenshot failed: ${e}`);
  }

  const { PlaywrightAgent } = await import('@midscene/web/playwright');
  const agent = new PlaywrightAgent(page);

  const prompt = `分析当前页面的截图。只描述你实际看到的内容，不要编造：
- 标题: 页面的实际标题（看截图）
- 导航: 实际存在的顶部导航项（没有就不填）
- 主要功能: 实际看到的功能模块
- 是否需要登录: 看到登录框/用户名密码输入框=true，没看到=false
- 技术栈: 实际能推断出的前端技术（如Vue/React/jQuery/小程序等）
- 表单: 实际存在的表单（如登录表单的账号/密码字段）
- 关键元素: 实际可见的关键UI元素
- 链接: 实际存在的内部链接（相对路径或同类域名）

JSON格式返回（不要返回空数组，字段为空就用[]）：
{title, primaryNav, mainFeatures, authRequired, techStack, forms, keyElements, links}`;

  let rawInfo: RawPageInfo;
  try {
    rawInfo = await agent.aiQuery<RawPageInfo>(prompt, { screenshotIncluded: true });
  } catch (e) {
    throw new Error(`aiQuery failed: ${e}`);
  }

  console.error(`[explorer] AI response: title="${rawInfo?.title}", authRequired=${rawInfo?.authRequired}, links_count=${Array.isArray(rawInfo?.links) ? rawInfo.links.length : '?'}`);

  return validatePageInfo(rawInfo, pageUrl, pageTitle);
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