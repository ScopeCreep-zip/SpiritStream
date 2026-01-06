import puppeteer from 'puppeteer';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { mkdirSync } from 'fs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const screenshotDir = join(__dirname, '..', 'screenshots', 'theme-testing');

async function testThemes() {
  const browser = await puppeteer.launch({
    headless: false,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
    defaultViewport: { width: 1400, height: 900 }
  });

  const page = await browser.newPage();

  try {
    console.log('Navigating to localhost:1420...');
    await page.goto('http://localhost:1420', { waitUntil: 'networkidle0', timeout: 30000 });
    await new Promise(r => setTimeout(r, 1000));

    // Navigate to Settings view
    console.log('Navigating to Settings view...');
    await page.evaluate(() => {
      const buttons = document.querySelectorAll('aside button');
      for (const btn of buttons) {
        if (btn.textContent.includes('Settings')) {
          btn.click();
          break;
        }
      }
    });
    await new Promise(r => setTimeout(r, 500));

    // Take screenshot of Settings view before opening dropdown
    console.log('Capturing Settings view...');
    await page.screenshot({
      path: join(screenshotDir, '01-settings-view.png'),
      fullPage: false
    });

    // Find and click the theme dropdown
    console.log('Opening theme dropdown...');
    await page.evaluate(() => {
      // Look for the theme select element
      const selects = document.querySelectorAll('select');
      for (const select of selects) {
        const label = select.previousElementSibling;
        if (label && label.textContent.includes('Theme')) {
          select.focus();
          select.click();
          return;
        }
      }
    });
    await new Promise(r => setTimeout(r, 500));

    // Capture dropdown open
    console.log('Capturing theme dropdown...');
    await page.screenshot({
      path: join(screenshotDir, '02-theme-dropdown-open.png'),
      fullPage: false
    });

    // Get all available themes
    const themes = await page.evaluate(() => {
      const selects = document.querySelectorAll('select');
      for (const select of selects) {
        const label = select.previousElementSibling;
        if (label && label.textContent.includes('Theme')) {
          const options = Array.from(select.options).map(opt => opt.value);
          return options;
        }
      }
      return [];
    });

    console.log('Available themes:', themes);

    // Test each theme
    const themeNames = {
      'spirit': '03-theme-spirit',
      'rainbow-pride': '04-theme-rainbow-pride',
      'trans-pride': '05-theme-trans-pride',
      'dracula': '06-theme-dracula',
      'nord': '07-theme-nord',
      'catppuccin-mocha': '08-theme-catppuccin-mocha'
    };

    for (const [themeId, filename] of Object.entries(themeNames)) {
      if (themes.includes(themeId)) {
        console.log(`Testing theme: ${themeId}...`);

        // Select the theme
        await page.evaluate((id) => {
          const selects = document.querySelectorAll('select');
          for (const select of selects) {
            const label = select.previousElementSibling;
            if (label && label.textContent.includes('Theme')) {
              select.value = id;
              select.dispatchEvent(new Event('change', { bubbles: true }));
              return;
            }
          }
        }, themeId);

        await new Promise(r => setTimeout(r, 800)); // Wait for theme to apply

        // Capture screenshot
        await page.screenshot({
          path: join(screenshotDir, `${filename}.png`),
          fullPage: false
        });
        console.log(`Captured: ${themeId}`);
      } else {
        console.log(`Theme ${themeId} not found in dropdown`);
      }
    }

    // Check console for errors
    console.log('\nChecking for console errors...');
    const consoleLogs = await page.evaluate(() => {
      return window.__consoleErrors || [];
    });

  } catch (error) {
    console.error('Error:', error.message);
    console.error(error.stack);
  } finally {
    console.log('\nTest complete. Press Ctrl+C to close browser.');
    // Keep browser open for manual inspection
    await new Promise(r => setTimeout(r, 60000));
    await browser.close();
  }
}

try {
  mkdirSync(screenshotDir, { recursive: true });
  console.log(`Screenshots will be saved to: ${screenshotDir}`);
} catch (e) {
  console.error('Failed to create screenshot directory:', e);
}

testThemes();
