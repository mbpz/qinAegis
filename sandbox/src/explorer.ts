import { chromium, Page } from 'playwright';

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

function isString(v: unknown): v is string {
  return typeof v === 'string';
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

function validatePageInfo(raw: unknown, pageUrl: string, pageTitle: string): PageInfo {
  const r = raw as Record<string, unknown> | undefined;
  const host = (() => { try { return new URL(pageUrl).host; } catch { return ''; } })();

  const links = r?.links;
  const validatedLinks: string[] = [];
  if (isStringArray(links)) {
    for (const l of links) {
      if (isValidLink(l, host)) validatedLinks.push(l);
    }
  }

  return {
    url: pageUrl,
    title: isString(r?.title) ? r.title : (pageTitle || ''),
    primaryNav: isStringArray(r?.primaryNav) ? r.primaryNav : [],
    mainFeatures: isStringArray(r?.mainFeatures) ? r.mainFeatures : [],
    authRequired: r?.authRequired === true,
    techStack: isStringArray(r?.techStack) ? r.techStack : [],
    forms: isFormInfoArray(r?.forms) ? r.forms : [],
    keyElements: isStringArray(r?.keyElements) ? r.keyElements : [],
    links: validatedLinks,
  };
}

interface ExtractedData {
  title: string;
  primaryNav: string[];
  authRequired: boolean;
  techStack: string[];
  forms: FormInfo[];
  links: string[];
}

async function extractFromDom(page: Page): Promise<ExtractedData> {
  return await page.evaluate(() => {
    const title = document.title || '';

    const navSelectors = [
      'header nav a', 'header .nav a', '.header-nav a', '.top-nav a',
      '.navbar a', '.nav-bar a', 'nav a', '.menu a',
    ];
    const primaryNav: string[] = [];
    for (const sel of navSelectors) {
      const els = document.querySelectorAll(sel);
      for (const el of els) {
        const text = el.innerText?.trim();
        if (text && text.length > 0 && text.length < 50 && !primaryNav.includes(text)) {
          primaryNav.push(text);
        }
      }
      if (primaryNav.length > 0) break;
    }

    const passwordInputs = document.querySelectorAll('input[type="password"], input[name*="password" i], input[name*="pwd" i]');
    const usernameInputs = document.querySelectorAll('input[name*="user" i], input[name*="account" i], input[name*="phone" i], input[name*="email" i]');
    const authRequired = passwordInputs.length > 0 && usernameInputs.length > 0;

    const forms: { action: string; method: string; fields: string[] }[] = [];
    for (const form of document.querySelectorAll('form')) {
      const action = form.action || '';
      const method = (form.method || 'get').toUpperCase();
      const fields: string[] = [];
      for (const inp of form.querySelectorAll('input, textarea, select')) {
        const name = (inp as HTMLInputElement).name || inp.id || '';
        const type = (inp as HTMLInputElement).type || '';
        if (name) fields.push(`${name}(${type})`);
      }
      if (action || fields.length > 0) {
        forms.push({ action, method, fields });
      }
    }

    const links: string[] = [];
    const sameHost = window.location.hostname;
    for (const a of document.querySelectorAll('a[href]')) {
      const href = a.href;
      if (!href) continue;
      try {
        const url = new URL(href);
        if (url.hostname === sameHost || !href.startsWith('http')) {
          links.push(href);
        }
      } catch {}
    }

    const techStack: string[] = [];
    const html = document.documentElement.innerHTML;
    if (html.includes('vue') || html.includes('data-v-')) techStack.push('Vue');
    if (html.includes('react') || html.includes('data-reactid')) techStack.push('React');
    if (html.includes('angular') || html.includes('ng-app')) techStack.push('Angular');
    if (html.includes('jquery')) techStack.push('jQuery');
    if (html.includes('next.js') || html.includes('__NEXT_DATA__')) techStack.push('Next.js');
    if (html.includes('nuxt')) techStack.push('Nuxt');
    if (html.includes('miniprogram')) techStack.push('微信小程序');
    if (html.includes('vant') || html.includes('vant-')) techStack.push('Vant');
    if (html.includes('element-ui') || html.includes('el-')) techStack.push('Element UI');

    return { title, primaryNav, authRequired, techStack, forms, links };
  });
}

export async function exploreProject(seedUrls: string[], maxDepth: number): Promise<PageInfo[]> {
  const visited = new Set<string>();
  const results: PageInfo[] = [];
  const queue: { url: string; depth: number }[] = seedUrls.map(u => ({ url: u, depth: 0 }));

  console.error(`[explorer] Launching Chromium on port ${CDP_PORT}...`);

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

  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  let page: Page;
  try {
    page = await context.newPage();
    console.error(`[explorer] Created page`);
  } catch (e) {
    throw new Error(`Failed to create page: ${e}`);
  }

  while (queue.length > 0) {
    const { url, depth } = queue.shift()!;
    if (visited.has(url) || depth > maxDepth) continue;
    visited.add(url);

    try {
      console.error(`[explorer] Navigating to ${url}...`);
      const response = await page.goto(url, { timeout: 30000, waitUntil: 'domcontentloaded' });
      console.error(`[explorer] HTTP status: ${response?.status()}, URL: ${page.url()}`);

      try {
        await page.waitForLoadState('networkidle', { timeout: 15000 });
      } catch (e) {
        console.error(`[explorer] networkidle timeout: ${e}`);
      }
      await page.waitForTimeout(2000);

      const pageTitle = await page.title();
      const pageContent = await page.content();
      console.error(`[explorer] title="${pageTitle}", html_len=${pageContent.length}`);

      const bodyText = await page.evaluate(() => document.body?.innerText?.slice(0, 200) || 'EMPTY');
      console.error(`[explorer] Body text: "${bodyText.replace(/\n/g, ' ').slice(0, 100)}"`);

      console.error(`[explorer] Extracting page info from DOM...`);
      try {
        const extracted = await extractFromDom(page);
        console.error(`[explorer] DOM extracted: title="${extracted.title}", authRequired=${extracted.authRequired}, nav_count=${extracted.primaryNav.length}, links_count=${extracted.links.length}`);

        const rawInfo = {
          title: extracted.title,
          primaryNav: extracted.primaryNav,
          mainFeatures: [] as string[],
          authRequired: extracted.authRequired,
          techStack: extracted.techStack,
          forms: extracted.forms,
          keyElements: [] as string[],
          links: extracted.links,
        };

        const info = validatePageInfo(rawInfo, page.url(), pageTitle);
        results.push(info);

        for (const link of info.links.slice(0, 10)) {
          const absUrl = new URL(link, url).href;
          if (!visited.has(absUrl)) {
            queue.push({ url: absUrl, depth: depth + 1 });
          }
        }
      } catch (e) {
        console.error(`[explorer] DOM extraction failed for ${url}: ${e}`);
      }
    } catch (e) {
      console.error(`[explorer] Navigation failed for ${url}: ${e}`);
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