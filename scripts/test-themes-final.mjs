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
  await page.setViewport({ width: 1600, height: 1000 });

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
    await new Promise(r => setTimeout(r, 1500));

    // Take screenshot of Settings view
    await page.screenshot({
      path: join(screenshotDir, '01-settings-view.png'),
      fullPage: true
    });
    console.log('✓ Captured Settings view');

    // Find the theme select - it's in the Themes card
    const themeInfo = await page.evaluate(() => {
      // Find all select elements
      const selects = Array.from(document.querySelectorAll('select'));

      // The theme select should be the second select (first is language, second is theme)
      // Or we can find it by checking if options contain theme names
      for (const select of selects) {
        const options = Array.from(select.options);
        // Check if any option value looks like a theme ID
        if (options.some(opt => ['spirit', 'dracula', 'nord'].includes(opt.value))) {
          return {
            found: true,
            currentValue: select.value,
            options: options.map(opt => ({
              value: opt.value,
              text: opt.text
            }))
          };
        }
      }

      return { found: false, selects: selects.length };
    });

    console.log('\nTheme selector info:', JSON.stringify(themeInfo, null, 2));

    if (!themeInfo.found) {
      console.log('❌ Could not find theme selector!');
      console.log(`   Found ${themeInfo.selects} select elements total`);
      return;
    }

    console.log(`\n✓ Found ${themeInfo.options.length} themes available:`);
    themeInfo.options.forEach((opt, i) => {
      const marker = opt.value === themeInfo.currentValue ? '→' : ' ';
      console.log(`  ${marker} ${i + 1}. ${opt.text} (${opt.value})`);
    });

    // Test each theme
    for (let i = 0; i < themeInfo.options.length; i++) {
      const theme = themeInfo.options[i];
      console.log(`\n[${i + 1}/${themeInfo.options.length}] Testing theme: ${theme.text}...`);

      // Select the theme
      await page.evaluate((value) => {
        const selects = Array.from(document.querySelectorAll('select'));
        for (const select of selects) {
          const options = Array.from(select.options);
          if (options.some(opt => ['spirit', 'dracula', 'nord'].includes(opt.value))) {
            select.value = value;
            select.dispatchEvent(new Event('change', { bubbles: true }));
            return true;
          }
        }
        return false;
      }, theme.value);

      await new Promise(r => setTimeout(r, 1200)); // Wait for theme to apply

      // Capture screenshot
      const safeFilename = theme.value.replace(/[^a-z0-9-]/gi, '-').toLowerCase();
      const filename = `theme-${String(i + 2).padStart(2, '0')}-${safeFilename}.png`;
      await page.screenshot({
        path: join(screenshotDir, filename),
        fullPage: true
      });
      console.log(`  ✓ Captured screenshot: ${filename}`);
    }

    console.log('\n' + '='.repeat(60));
    console.log('✅ THEME TESTING COMPLETE!');
    console.log('='.repeat(60));
    console.log(`\nScreenshots saved to:\n${screenshotDir}`);
    console.log(`\nTotal screenshots: ${themeInfo.options.length + 1}`);

  } catch (error) {
    console.error('\n❌ Error:', error.message);
    console.error(error.stack);
  } finally {
    console.log('\nClosing browser in 3 seconds...');
    await new Promise(r => setTimeout(r, 3000));
    await browser.close();
  }
}

try {
  mkdirSync(screenshotDir, { recursive: true });
} catch (e) {}

console.log('SpiritStream Theme Testing');
console.log('='.repeat(60));
testThemes();
