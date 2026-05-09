// sandbox/src/executor.ts
// IMPORTANT: Override console methods BEFORE any other imports
const originalError = console.error;
const originalLog = console.log;
const originalWarn = console.warn;
console.error = (...args: any[]) => {
  const msg = args.map(a => typeof a === 'object' ? JSON.stringify(a) : String(a)).join(' ');
  // Only allow our own debug messages through
  if (msg.includes('[executor]') || msg.includes('[midscene]')) {
    originalError.apply(console, args);
  }
};
console.log = () => {};  // Silence all console.log
console.warn = () => {}; // Silence all console.warn

import { chromium, Browser, Page, BrowserContext } from 'playwright';
import { PlaywrightAgent } from '@midscene/web/playwright';
import * as readline from 'readline';
import * as fs from 'fs';

interface JsonRpcRequest {
  id: string;
  method: string;
  args: unknown[];
}

let browser: Browser | null = null;
let context: BrowserContext | null = null;
let page: Page | null = null;
let agent: PlaywrightAgent | null = null;

const CDP_PORT = parseInt(process.env.CDP_PORT || '9222', 10);
const DEBUG = process.env.DEBUG === '1';

// Debug log that writes to stderr via native syscall
function debug(...args: any[]) {
  if (DEBUG) {
    const msg = args.map(a => typeof a === 'object' ? JSON.stringify(a) : String(a)).join(' ');
    fs.writeSync(2, msg + '\n');
  }
}

async function ensureBrowser() {
  if (!browser) {
    debug(`[executor] Launching Chromium on port ${CDP_PORT}...`);

    browser = await chromium.launch({
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

    context = await browser.newContext({
      ignoreHTTPSErrors: true,
    });

    page = await context.newPage();
    agent = new PlaywrightAgent(page);

    debug(`[executor] Browser launched successfully`);
  }
}

async function ensureConnected() {
  await ensureBrowser();
}

async function handleRequest(req: JsonRpcRequest): Promise<unknown> {
  try {
    if (!['explore', 'lighthouse', 'stress'].includes(req.method)) {
      await ensureConnected();
    }

    switch (req.method) {
      case 'aiQuery': {
        const prompt = req.args as string;
        debug(`[executor] aiQuery: prompt length ${prompt.length}`);
        const data = await agent!.aiQuery(prompt);
        const dataStr = JSON.stringify(data);
        debug(`[executor] aiQuery: response length ${dataStr.length}`);
        return { id: req.id, ok: true, data: dataStr };
      }
      case 'aiAct': {
        const action = req.args as string;
        await agent!.aiAct(action);
        return { id: req.id, ok: true, data: null };
      }
      case 'aiAssert': {
        const assertion = req.args as string;
        await agent!.aiAssert(assertion);
        return { id: req.id, ok: true, data: null };
      }
      case 'goto': {
        const { url } = req.args as { url: string };
        await page!.goto(url);
        return { id: req.id, ok: true, data: null };
      }
      case 'screenshot': {
        const buf = await page!.screenshot({ encoding: 'base64' } as any);
        return { id: req.id, ok: true, data: buf };
      }
      case 'explore': {
        const { url, depth } = req.args as { url: string; depth: number };
        const { exploreProject, toMarkdown } = await import('./explorer.js');
        const pages = await exploreProject([url], depth);
        const md = toMarkdown(pages);
        return { id: req.id, ok: true, data: { pages, markdown: md } };
      }
      case 'run_yaml': {
        const { yaml_script, case_id } = req.args as { yaml_script: string; case_id: string };
        if (!browser) await ensureConnected();
        const testContext = await browser!.newContext();
        const testPage = await testContext.newPage();
        try {
          const { runYaml } = await import('./yaml_runner.js');
          const result = await runYaml(yaml_script, case_id, testPage);
          return { id: req.id, ok: true, data: result };
        } finally {
          await testPage.close();
          await testContext.close();
        }
      }
      case 'lighthouse': {
        const { url } = req.args as { url: string };
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
        const { target_url, users, spawn_rate, duration } = req.args as { target_url: string; users: number; spawn_rate: number; duration: number };
        const { runLocust } = await import('./locust_runner.js');
        const result = await runLocust(target_url, users, spawn_rate, duration);
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
    fs.writeSync(2, `[executor] Received request id: '${req.id}', method: ${req.method}\n`);
    const resp = await handleRequest(req);
    const respJson = JSON.stringify(resp);
    fs.writeSync(2, `[executor] Response JSON (${respJson.length} chars): ${respJson}\n`);
    // ONLY write JSON to stdout - no other output
    process.stdout.write(respJson + '\n');
  } catch (e) {
    const errResp = JSON.stringify({ id: '?', ok: false, error: String(e) });
    process.stdout.write(errResp + '\n');
  }
});
