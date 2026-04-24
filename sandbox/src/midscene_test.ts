import { chromium } from 'playwright';
import { PlaywrightAgent } from '@midscene/web/playwright';

async function main() {
  const cdpUrl = process.env.CDP_WS_URL || 'ws://localhost:9222';

  console.log(`Connecting to CDP at ${cdpUrl}...`);
  const browser = await chromium.connectOverCDP(cdpUrl);
  const page = await browser.newPage();

  const agent = new PlaywrightAgent(page);

  // Navigate to example site
  await page.goto('https://example.com');

  // AI query to extract page info
  const pageInfo = await agent.aiQuery<{
    title: string;
    description: string;
    hasLoginForm: boolean;
  }>(
    '{title: string, description: string, hasLoginForm: boolean}, ' +
    '提取页面标题、描述，判断是否有登录表单'
  );

  console.log('Page info:', JSON.stringify(pageInfo, null, 2));

  // AI assert
  await agent.aiAssert('页面加载完成，显示了 Example Domain 标题');

  console.log('✓ Midscene smoke test passed');
  await browser.close();
}

main().catch((e) => {
  console.error('Test failed:', e);
  process.exit(1);
});
