// sandbox/src/executor.ts
import { chromium, Browser, Page } from 'playwright';
import { PlaywrightAgent } from '@midscene/web/playwright';
import * as readline from 'readline';

interface JsonRpcRequest {
  id: string;
  method: string;
  args: unknown[];
}

let browser: Browser | null = null;
let page: Page | null = null;
let agent: PlaywrightAgent | null = null;

const CDP_HOST = process.env.CDP_HOST || 'localhost';
const CDP_PORT = process.env.CDP_PORT || '9222';
const CDP_URL = `ws://${CDP_HOST}:${CDP_PORT}`;

async function resolveCdpUrl(): Promise<string> {
  try {
    // Get browser WebSocket URL from /json/version
    const response = await fetch(`http://${CDP_HOST}:${CDP_PORT}/json/version`);
    if (!response.ok) {
      throw new Error(`CDP /json/version returned ${response.status}`);
    }
    const info = await response.json();
    console.error(`[executor] Connecting to browser: ${info.webSocketDebuggerUrl}`);
    return info.webSocketDebuggerUrl;
  } catch (e) {
    throw new Error(`Failed to resolve CDP URL: ${e}`);
  }
}

async function ensureConnected() {
  if (!browser) {
    const wsUrl = await resolveCdpUrl();
    console.error(`[executor] Connecting to CDP via: ${wsUrl}`);
    browser = await chromium.connectOverCDP(wsUrl);
    page = await browser.newPage();
    agent = new PlaywrightAgent(page);
  }
}

async function handleRequest(req: JsonRpcRequest): Promise<unknown> {
  try {
    // Only ensure connected for methods that need the executor's page/agent
    // 'explore' creates its own browser connection in explorer.ts
    if (req.method !== 'explore') {
      await ensureConnected();
    }

    switch (req.method) {
      case 'aiQuery': {
        const [prompt] = req.args as [string];
        const data = await agent!.aiQuery(prompt);
        return { id: req.id, ok: true, data };
      }
      case 'aiAct': {
        const [action] = req.args as [string];
        await agent!.aiAct(action);
        return { id: req.id, ok: true, data: null };
      }
      case 'aiAssert': {
        const [assertion] = req.args as [string];
        await agent!.aiAssert(assertion);
        return { id: req.id, ok: true, data: null };
      }
      case 'goto': {
        const args = req.args as [{ url: string }];
        const url = args[0].url;
        await page!.goto(url);
        return { id: req.id, ok: true, data: null };
      }
      case 'screenshot': {
        const buf = await page!.screenshot({ encoding: 'base64' } as any);
        return { id: req.id, ok: true, data: buf };
      }
      case 'explore': {
        // Rust serializes Explore { url, depth } as {"url":..., "depth":...} object
        const args = req.args as { url: string; depth: number };
        const { exploreProject, toMarkdown } = await import('./explorer.js');
        const pages = await exploreProject([args.url], args.depth);
        const md = toMarkdown(pages);
        return { id: req.id, ok: true, data: { pages, markdown: md } };
      }
      case 'run_yaml': {
        const [yamlScript, caseId] = req.args as [string, string];
        if (!browser) await ensureConnected();
        const runPage = await browser!.newPage();
        try {
          const { runYaml } = await import('./yaml_runner.js');
          const result = await runYaml(yamlScript, caseId, runPage);
          return { id: req.id, ok: true, data: result };
        } finally {
          await runPage.close();
        }
      }
      case 'lighthouse': {
        const [url] = req.args as [string];
        // Validate URL before passing to runLighthouse
        try {
          new URL(url);
          if (!['http:', 'https:'].includes(new URL(url).protocol)) {
            throw new Error('Only http/https URLs are allowed');
          }
        } catch (e) {
          return { id: req.id, ok: false, error: `Invalid URL: ${url}` };
        }
        const outputPath = `/tmp/lighthouse_${Date.now()}.json`;
        const { runLighthouse } = await import('./lighthouse_runner.js');
        const result = await runLighthouse(url, outputPath);
        return { id: req.id, ok: true, data: result };
      }
      case 'stress': {
        const [targetUrl, users, spawnRate, duration] = req.args as [string, number, number, number];
        const { runLocust } = await import('./locust_runner.js');
        const result = await runLocust(targetUrl, users, spawnRate, duration);
        return { id: req.id, ok: true, data: result };
      }
      case 'shutdown': {
        await browser?.close();
        process.exit(0);
      }
      default:
        return { id: req.id, ok: false, error: `Unknown method: ${req.method}` };
    }
  } catch (e) {
    return { id: req.id, ok: false, error: String(e) };
  }
}

const rl = readline.createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
});

rl.on('line', async (line) => {
  if (!line.trim()) return;
  try {
    const req: JsonRpcRequest = JSON.parse(line);
    const resp = await handleRequest(req);
    console.log(JSON.stringify(resp));
  } catch (e) {
    console.error(JSON.stringify({ id: '?', ok: false, error: String(e) }));
  }
});
