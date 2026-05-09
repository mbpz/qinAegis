// sandbox/src/executor.ts
import { chromium, Browser, Page, BrowserContext } from 'playwright';
import { PlaywrightAgent } from '@midscene/web/playwright';
import * as readline from 'readline';

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

async function ensureBrowser() {
  if (!browser) {
    console.error(`[executor] Launching Chromium on port ${CDP_PORT}...`);

    // Launch browser with headless mode and debugging port
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

    // Create isolated context for each session
    context = await browser.newContext({
      // No stored state - fresh browser
      ignoreHTTPSErrors: true,
    });

    page = await context.newPage();
    agent = new PlaywrightAgent(page);

    console.error(`[executor] Browser launched successfully`);
  }
}

async function ensureConnected() {
  await ensureBrowser();
}

async function handleRequest(req: JsonRpcRequest): Promise<unknown> {
  try {
    // Only ensure connected for methods that need the executor's page/agent
    // 'explore' creates its own browser connection in explorer.ts
    // 'lighthouse' and 'stress' run external tools (don't need Playwright browser)
    if (!['explore', 'lighthouse', 'stress'].includes(req.method)) {
      await ensureConnected();
    }

    switch (req.method) {
      case 'aiQuery': {
        // Rust AiQuery(String) expects a JSON string response
        console.error(`[executor] aiQuery: Starting...`);
        const prompt = req.args as string;
        console.error(`[executor] aiQuery: Calling agent.aiQuery with prompt length ${prompt.length}`);
        const data = await agent!.aiQuery(prompt);
        // Midscene returns an object, but Rust expects a JSON string
        const dataStr = JSON.stringify(data);
        console.error(`[executor] aiQuery: Got response, data length: ${dataStr.length}`);
        return { id: req.id, ok: true, data: dataStr };
      }
      case 'aiAct': {
        // Rust AiAct(String) serializes args as a plain string
        const action = req.args as string;
        await agent!.aiAct(action);
        return { id: req.id, ok: true, data: null };
      }
      case 'aiAssert': {
        // Rust AiAssert(String) serializes args as a plain string
        const assertion = req.args as string;
        await agent!.aiAssert(assertion);
        return { id: req.id, ok: true, data: null };
      }
      case 'goto': {
        // Rust Goto { url } serializes args as { url: "..." }
        const { url } = req.args as { url: string };
        await page!.goto(url);
        return { id: req.id, ok: true, data: null };
      }
      case 'screenshot': {
        const buf = await page!.screenshot({ encoding: 'base64' } as any);
        return { id: req.id, ok: true, data: buf };
      }
      case 'explore': {
        // Rust serializes Explore { url, depth } as {"url":..., "depth":...} object
        const { url, depth } = req.args as { url: string; depth: number };
        const { exploreProject, toMarkdown } = await import('./explorer.js');
        const pages = await exploreProject([url], depth);
        const md = toMarkdown(pages);
        return { id: req.id, ok: true, data: { pages, markdown: md } };
      }
      case 'run_yaml': {
        // Rust RunYaml { yaml_script, case_id } serializes as object
        const { yaml_script, case_id } = req.args as { yaml_script: string; case_id: string };
        if (!browser) await ensureConnected();
        // Create new isolated context for each test
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
    const resp = await handleRequest(req);
    const respJson = JSON.stringify(resp);
    console.error(`[executor] Sending response: ${respJson.substring(0, 100)}...`);
    console.log(respJson);
    // Flush stdout to ensure response is sent
    process.stdout.flush();
  } catch (e) {
    console.error(JSON.stringify({ id: '?', ok: false, error: String(e) }));
  }
});