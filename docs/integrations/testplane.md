# Testplane Integration

## Overview

[Testplane](https://github.com/testplane/testplane) is an open-source visual regression testing framework by [Skypoint](https://www.skypoint.dev/). It specializes in AI-powered visual testing and can complement qinAegis for visual regression scenarios.

## Features

- **AI-Powered Comparison**: Intelligent visual diff detection
- **Component Testing**: Test UI components in isolation
- **Cross-browser Testing**: Run visual tests across browsers
- **CI/CD Integration**: GitHub Actions, GitLab CI, Jenkins
- **Interactive Review**: Web-based diff review tool

## Installation

```bash
npm install -D @testplane/testplane @testplane/plugin-hermione

# Or with pnpm
pnpm add -D @testplane/testplane @testplane/plugin-hermione
```

## Basic Usage

### 1. Configuration

```javascript
// .testplane.js
module.exports = {
  sets: [
    {
      name: 'desktop',
      files: 'tests/**/*.testplane.js',
      browser: 'chromium',
      viewport: { width: 1440, height: 900 },
    },
  ],
  plugins: [
    ['@testplane/plugin-hermione', {
      project: 'your-project-token',
    }],
  ],
  gemini: {
    apiKey: process.env.GEMINI_API_KEY,
    threshold: 0.1, // 10% visual difference threshold
  },
};
```

### 2. Visual Test

```javascript
// tests/example.testplane.js
const { TestplaneTest } = require('@testplane/testplane');

TestplaneTest('Visual regression - homepage', async ({ browser }) => {
  const page = await browser.newPage();
  await page.goto('https://your-app.com');

  // Capture screenshot
  const screenshot = await page.locator('body').screenshot();

  // Compare with baseline
  await expect(screenshot).toMatchImageSnapshot({
    baseline: 'homepage-desktop.png',
    threshold: 0.1,
  });
});
```

## Integration with qinAegis

### 1. Add to Project

```bash
cd sandbox
pnpm add @testplane/testplane @testplane/plugin-hermione
```

### 2. Create Visual Testing Module

```typescript
// sandbox/src/visual-tester.ts

interface VisualTestConfig {
  projectName: string;
  apiKey: string;
  threshold: number;
  viewport?: { width: number; height: number };
}

interface VisualTestResult {
  passed: boolean;
  diff?: {
    id: string;
    diffImage: string;
    baselineImage: string;
    currentImage: string;
    diffPercent: number;
  };
  error?: string;
}

export async function runVisualTest(
  url: string,
  selector: string,
  config: VisualTestConfig
): Promise<VisualTestResult> {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();

  if (config.viewport) {
    await page.setViewportSize(config.viewport);
  }

  try {
    await page.goto(url);
    const element = page.locator(selector);
    const screenshot = await element.screenshot();

    // Compare using local diffing
    const baselinePath = `.qinAegis/visual-baselines/${config.projectName}/${selector}.png`;

    if (fs.existsSync(baselinePath)) {
      const baseline = fs.readFileSync(baselinePath);
      const diffPercent = computeDiff(screenshot, baseline);

      if (diffPercent > config.threshold) {
        return {
          passed: false,
          diff: {
            id: `${config.projectName}/${selector}`,
            diffImage: generateDiffImage(screenshot, baseline),
            baselineImage: baselinePath,
            currentImage: screenshot,
            diffPercent,
          },
        };
      }
    } else {
      // Create baseline
      fs.mkdirSync(path.dirname(baselinePath), { recursive: true });
      fs.writeFileSync(baselinePath, screenshot);
    }

    return { passed: true };
  } catch (error) {
    return { passed: false, error: String(error) };
  } finally {
    await browser.close();
  }
}
```

### 3. Integrate with Reporter

```rust
// crates/core/src/reporter.rs

pub fn generate_visual_report(
    &self,
    project_name: &str,
    visual_results: &[VisualTestResult],
) -> anyhow::Result<PathBuf> {
    let dir = Self::report_dir("visual");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("visual-report.html");

    let html = render_visual_report_html(project_name, visual_results);
    std::fs::write(&path, html)?;

    Ok(path)
}
```

## Visual Testing Workflow

```
1. BASELINE CREATION
   └── Run tests with --update-baselines
       └── Screenshots stored in .qinAegis/visual-baselines/

2. TEST EXECUTION
   └── Run visual tests via qinAegis run --test-type visual
       └── Compare current screenshots with baselines
       └── Generate diff images if differences found

3. REVIEW (if differences found)
   └── Review diffs in TUI
   └── Accept: Update baseline
   └── Reject: Flag as product issue

4. GATE
   └── If diff% > threshold, fail the gate
   └── Generate visual report
```

## Commands

```bash
# Run visual tests
qinAegis run --project myapp --test-type visual

# Update baselines
qinAegis run --project myapp --test-type visual --update-baselines

# View visual report
qinAegis report --type visual

# Run with specific viewport
qinAegis run --project myapp --test-type visual --viewport 1920x1080
```

## Resources

- [Testplane GitHub](https://github.com/testplane/testplane)
- [Testplane Documentation](https://testplane.org/)
