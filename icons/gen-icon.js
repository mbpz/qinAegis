const { chromium } = require('/Users/jinguo.zeng/.n/lib/node_modules/playwright');
const fs = require('fs');
const svgPath = process.argv[2];
const iconsetDir = process.argv[3];

const svg = fs.readFileSync(svgPath, 'utf8');

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();

  const sizes = [16, 32, 64, 128, 256, 512, 1024];
  for (const size of sizes) {
    await page.setViewportSize({ width: size, height: size });
    await page.setContent('<html><body style="margin:0;background:white;">' + svg + '</body></html>');
    await page.screenshot({ path: iconsetDir + '/icon_' + size + 'x' + size + '.png', width: size, height: size });
  }

  // 512@2x for Retina
  await page.setViewportSize({ width: 1024, height: 1024 });
  await page.setContent('<html><body style="margin:0;background:white;">' + svg + '</body></html>');
  await page.screenshot({ path: iconsetDir + '/icon_512x512@2x.png', width: 1024, height: 1024 });

  await browser.close();
  console.log('icons generated');
})().catch(e => { console.error(e); process.exit(1); });