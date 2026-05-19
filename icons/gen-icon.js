const fs = require('fs');
const path = require('path');

let chromium;
try {
  chromium = require('playwright').chromium;
} catch {
  chromium = require(path.join(
    process.env.HOME || '',
    '.n/lib/node_modules/playwright'
  )).chromium;
}

const svgPath = process.argv[2];
const iconsetDir = process.argv[3];

if (!svgPath || !iconsetDir) {
  console.error('Usage: node gen-icon.js <icon.svg> <iconset-dir>');
  process.exit(1);
}

// macOS iconutil requires exact filenames and pixel dimensions per slot.
const ICONSET_FILES = [
  { file: 'icon_16x16.png', size: 16 },
  { file: 'icon_16x16@2x.png', size: 32 },
  { file: 'icon_32x32.png', size: 32 },
  { file: 'icon_32x32@2x.png', size: 64 },
  { file: 'icon_128x128.png', size: 128 },
  { file: 'icon_128x128@2x.png', size: 256 },
  { file: 'icon_256x256.png', size: 256 },
  { file: 'icon_256x256@2x.png', size: 512 },
  { file: 'icon_512x512.png', size: 512 },
  { file: 'icon_512x512@2x.png', size: 1024 },
];

const rawSvg = fs.readFileSync(svgPath, 'utf8');
// Let CSS scale the SVG to the viewport; fixed width/height causes top-left clipping.
const scalableSvg = rawSvg.replace(/\s(width|height)="[^"]*"/gi, '');

function renderPage(size) {
  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  html, body {
    width: ${size}px;
    height: ${size}px;
    overflow: hidden;
    background: #1A237E;
  }
  body {
    display: flex;
    align-items: center;
    justify-content: center;
  }
  svg {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
</head>
<body>${scalableSvg}</body>
</html>`;
}

(async () => {
  fs.mkdirSync(iconsetDir, { recursive: true });

  const browser = await chromium.launch();
  const page = await browser.newPage();

  for (const { file, size } of ICONSET_FILES) {
    await page.setViewportSize({ width: size, height: size });
    await page.setContent(renderPage(size), { waitUntil: 'networkidle' });
    await page.screenshot({
      path: path.join(iconsetDir, file),
      type: 'png',
      omitBackground: false,
    });
    console.log(`  ${file} (${size}x${size})`);
  }

  await browser.close();
  console.log('icons generated');
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
