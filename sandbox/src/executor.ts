import { PlaywrightAgent } from '@midscene/web/playwright';
import { chromium, Browser, Page } from 'playwright';

export class MidsceneExecutor {
  private browser: Browser | null = null;
  private page: Page | null = null;
  private agent: PlaywrightAgent | null = null;

  async connect(cdpUrl: string): Promise<void> {
    this.browser = await chromium.connectOverCDP(cdpUrl);
    this.page = await this.browser.newPage();
    this.agent = new PlaywrightAgent(this.page);
  }

  async aiAct(action: string): Promise<void> {
    if (!this.agent) throw new Error('Not connected');
    await this.agent.aiAct(action);
  }

  async aiQuery<T>(prompt: string): Promise<T> {
    if (!this.agent) throw new Error('Not connected');
    return await this.agent.aiQuery<T>(prompt);
  }

  async aiAssert(assertion: string): Promise<void> {
    if (!this.agent) throw new Error('Not connected');
    await this.agent.aiAssert(assertion);
  }

  async close(): Promise<void> {
    await this.browser?.close();
  }
}
