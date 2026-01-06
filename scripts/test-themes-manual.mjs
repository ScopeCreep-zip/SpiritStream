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
    defaultViewport: null
  });

  const page = await browser.newPage();

  try {
    console.log('Navigating to localhost:1420...');
    await page.goto('http://localhost:1420', { waitUntil: 'networkidle0', timeout: 30000 });
    await new Promise(r => setTimeout(r, 2000));

    // Click on Settings in the SYSTEM section (last nav item)
    console.log('Clicking on Settings...');
    await page.evaluate(() => {
      const navButtons = document.querySelectorAll('aside button');
      // Get the last button which should be Settings
      const settingsBtn = navButtons[navButtons.length - 1];
      settingsBtn.click();
    });
    await new Promise(r => setTimeout(r, 1000));

    // Take screenshot of Settings view
    await page.screenshot({
      path: join(screenshotDir, '01-settings-view.png'),
      fullPage: true
    });
    console.log('Captured Settings view');

    // Find the theme select and get its options
    const themeInfo = await page.evaluate(() => {
      const labels = Array.from(document.querySelectorAll('label'));
      const themeLabel = labels.find(l => l.textContent.includes('Theme'));
      if (!themeLabel) return { found: false };

      // Find the select element after this label
      let el = themeLabel.nextElementSibling;
      while (el && el.tagName !== 'SELECT') {
        el = el.nextElementSibling;
      }

      if (!el) return { found: false };

      const options = Array.from(el.options).map(opt => ({
        value: opt.value,
        text: opt.text
      }));

      return {
        found: true,
        options,
        currentValue: el.value
      };
    });

    console.log('Theme selector info:', JSON.stringify(themeInfo, null, 2));

    if (!themeInfo.found) {
      console.log('Could not find theme selector!');
      return;
    }

    // Test each theme
    for (let i = 0; i < themeInfo.options.length; i++) {
      const theme = themeInfo.options[i];
      console.log(`\nTesting theme: ${theme.text} (${theme.value})`);

      // Select the theme
      await page.evaluate((value) => {
        const labels = Array.from(document.querySelectorAll('label'));
        const themeLabel = labels.find(l => l.textContent.includes('Theme'));
        let el = themeLabel.nextElementSibling;
        while (el && el.tagName !== 'SELECT') {
          el = el.nextElementSibling;
        }
        if (el) {
          el.value = value;
          el.dispatchEvent(new Event('change', { bubbles: true }));
        }
      }, theme.value);

      await new Promise(r => setTimeout(r, 1500)); // Wait for theme to apply

      // Capture screenshot
      const filename = `theme-${i + 1}-${theme.value.replace(/[^a-z0-9]/gi, '-')}.png`;
      await page.screenshot({
        path: join(screenshotDir, filename),
        fullPage: true
      });
      console.log(`Captured: ${theme.text}`);
    }

    console.log('\n✅ Theme testing complete!');
    console.log(`Screenshots saved to: ${screenshotDir}`);

  } catch (error) {
    console.error('❌ Error:', error.message);
    console.error(error.stack);
  } finally {
    console.log('\nClosing browser in 5 seconds...');
    await new Promise(r => setTimeout(r, 5000));
    await browser.close();
  }
}

try {
  mkdirSync(screenshotDir, { recursive: true });
} catch (e) {}

testThemes();
