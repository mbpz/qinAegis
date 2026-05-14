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
import { globalActionCache } from './action_cache.js';
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
const CDP_WS_URL = process.env.CDP_WS_URL;
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
    if (CDP_WS_URL) {
      debug(`[executor] Connecting to existing Chrome via CDP: ${CDP_WS_URL}`);
      browser = await chromium.connectOverCDP(CDP_WS_URL);
      context = await browser.newContext({
        ignoreHTTPSErrors: true,
      });
      page = await context.newPage();
      agent = new PlaywrightAgent(page);
      debug(`[executor] Connected to Chrome successfully`);
    } else {
      debug(`[executor] Launching fresh Chromium on port ${CDP_PORT}...`);
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
}

async function ensureConnected() {
  await ensureBrowser();
}

async function handleRequest(req: JsonRpcRequest): Promise<unknown> {
  try {
    if (!['explore', 'lighthouse', 'stress', 'zap_scan'].includes(req.method)) {
      await ensureConnected();
    } else {
      await ensureBrowser();
    }

    switch (req.method) {
      case 'aiQuery': {
        const prompt = req.args as string;
        debug(`[executor] aiQuery: prompt length ${prompt.length}`);
        // Check action cache first
        const cached = await globalActionCache.get(page!, prompt);
        if (cached !== null) {
          const dataStr = JSON.stringify(cached);
          debug(`[executor] aiQuery: CACHE HIT (${dataStr.length} chars)`);
          return { id: req.id || '0', ok: true, data: dataStr, cached: true };
        }
        // Include screenshot for page analysis
        const data = await agent!.aiQuery(prompt, { screenshotIncluded: true });
        const dataStr = JSON.stringify(data);
        debug(`[executor] aiQuery: response length ${dataStr.length}`);
        // Cache the result
        await globalActionCache.set(page!, prompt, data);
        return { id: req.id || '0', ok: true, data: dataStr };
      }
      case 'aiAct': {
        const action = req.args as string;
        // Check action cache first
        const cached = await globalActionCache.get(page!, 'act:' + action);
        if (cached !== null) {
          debug(`[executor] aiAct: CACHE HIT`);
          return { id: req.id || '0', ok: true, data: null, cached: true };
        }
        await agent!.aiAct(action);
        await globalActionCache.set(page!, 'act:' + action, true);
        return { id: req.id || '0', ok: true, data: null };
      }
      case 'aiAssert': {
        const assertion = req.args as string;
        // Check action cache first
        const cached = await globalActionCache.get(page!, 'assert:' + assertion);
        if (cached !== null) {
          debug(`[executor] aiAssert: CACHE HIT`);
          return { id: req.id || '0', ok: true, data: null, cached: true };
        }
        await agent!.aiAssert(assertion);
        await globalActionCache.set(page!, 'assert:' + assertion, true);
        return { id: req.id || '0', ok: true, data: null };
      }
      case 'goto': {
        const { url } = req.args as { url: string };
        await page!.goto(url);
        return { id: req.id || '0', ok: true, data: null };
      }
      case 'screenshot': {
        const buf = await page!.screenshot({ encoding: 'base64' } as any);
        return { id: req.id || '0', ok: true, data: buf };
      }
      case 'explore': {
        const { url, depth } = req.args as { url: string; depth: number };
        const { exploreProject, toMarkdown } = await import('./explorer.js');
        const pages = await exploreProject([url], depth);
        const md = toMarkdown(pages);
        return { id: req.id || '0', ok: true, data: { pages, markdown: md } };
      }
      case 'run_yaml': {
        const { yaml_script, case_id, target_url } = req.args as { yaml_script: string; case_id: string; target_url?: string };
        if (!browser) await ensureConnected();
        const testContext = await browser!.newContext();
        const testPage = await testContext.newPage();
        try {
          const { runYaml } = await import('./yaml_runner.js');
          const result = await runYaml(yaml_script, case_id, testPage, target_url);
          return { id: req.id || '0', ok: true, data: result };
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
          return { id: req.id || '0', ok: false, error: `Invalid URL: ${url}` };
        }
        const outputPath = `/tmp/lighthouse_${Date.now()}.json`;
        const { runLighthouse } = await import('./lighthouse_runner.js');
        const result = await runLighthouse(url, outputPath);
        return { id: req.id || '0', ok: true, data: result };
      }
      case 'stress': {
        const { target_url, users, spawn_rate, duration } = req.args as { target_url: string; users: number; spawn_rate: number; duration: number };
        const { runLocust } = await import('./locust_runner.js');
        const result = await runLocust(target_url, users, spawn_rate, duration);
        return { id: req.id || '0', ok: true, data: result };
      }
      case 'zap_scan': {
        const { target_url } = req.args as { target_url: string };
        try {
          new URL(target_url);
          if (!['http:', 'https:'].includes(new URL(target_url).protocol)) {
            throw new Error('Only http/https URLs are allowed');
          }
        } catch (e) {
          return { id: req.id || '0', ok: false, error: `Invalid URL: ${target_url}` };
        }
        const outputPath = `/tmp/zap_report_${Date.now()}.json`;
        const { runZapScan } = await import('./zap_runner.js');
        const result = await runZapScan(target_url, outputPath);
        return { id: req.id || '0', ok: true, data: result };
      }
      case 'shutdown': {
        await browser?.close();
        process.exit(0);
      }
      default:
        return { id: req.id || '0', ok: false, error: `Unknown method: ${req.method}` };
    }
  } catch (e) {
    return { id: req.id || '0', ok: false, error: String(e) };
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
