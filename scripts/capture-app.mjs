import puppeteer from 'puppeteer';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { mkdirSync } from 'fs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const screenshotDir = join(__dirname, '..', 'screenshots');

async function captureAllViews() {
  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox']
  });

  const page = await browser.newPage();
  await page.setViewport({ width: 1400, height: 900 });

  const views = ['Dashboard', 'Profiles', 'Stream Manager', 'Encoder Settings', 'Output Groups', 'Stream Targets', 'Logs', 'Settings'];

  try {
    await page.goto('http://localhost:1420', { waitUntil: 'networkidle0', timeout: 30000 });
    await new Promise(r => setTimeout(r, 500));

    for (const viewName of views) {
      // Click nav item by finding button with matching text
      await page.evaluate((name) => {
        const buttons = document.querySelectorAll('aside button');
        for (const btn of buttons) {
          if (btn.textContent.includes(name)) {
            btn.click();
            break;
          }
        }
      }, viewName);

      await new Promise(r => setTimeout(r, 400));

      const filename = viewName.toLowerCase().replace(/\s+/g, '-');
      await page.screenshot({
        path: join(screenshotDir, `view-${filename}.png`),
        fullPage: false
      });
      console.log(`Captured: ${viewName}`);
    }

  } catch (error) {
    console.error('Error:', error.message);
  } finally {
    await browser.close();
  }
}

try { mkdirSync(screenshotDir, { recursive: true }); } catch (e) {}
captureAllViews();
