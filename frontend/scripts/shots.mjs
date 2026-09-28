// Visual check: screenshots every screen in both themes at desktop and phone
// sizes, and reports console errors. Uses the Edge/Chrome already installed.
//
//   node scripts/shots.mjs [baseUrl] [outDir]
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "playwright-core";

const base = process.argv[2] ?? "https://localhost:3443";
const out = process.argv[3] ?? "shots";
const only = process.env.ONLY?.split(",");
mkdirSync(out, { recursive: true });

const browser = await chromium.launch({ channel: process.env.BROWSER ?? "msedge", headless: true });
const errors = [];

async function run(name, { width, height, mobile, theme }, fn) {
  if (only && !only.some((o) => name.includes(o))) return;
  const ctx = await browser.newContext({
    viewport: { width, height },
    deviceScaleFactor: mobile ? 2 : 1,
    isMobile: mobile,
    hasTouch: mobile,
    colorScheme: theme,
    ignoreHTTPSErrors: true,
    reducedMotion: process.env.REDUCED ? "reduce" : "no-preference",
  });
  const page = await ctx.newPage();
  page.on("console", (m) => m.type() === "error" && errors.push(`${name}: ${m.text()}`));
  page.on("pageerror", (e) => errors.push(`${name}: ${e.message}`));
  page.on("requestfailed", (r) => errors.push(`${name}: request failed ${r.url()} ${r.failure()?.errorText}`));
  try {
    await fn(page, (label) => page.screenshot({ path: join(out, `${name}-${label}.png`) }));
  } catch (e) {
    errors.push(`${name}: ${e.message}`);
    await page.screenshot({ path: join(out, `${name}-FAILED.png`) }).catch(() => {});
  }
  await ctx.close();
}

const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const sizes = {
  desktop: { width: 1440, height: 900, mobile: false },
  phone: { width: 390, height: 844, mobile: true },
};

for (const theme of ["dark", "light"]) {
  for (const [dev, size] of Object.entries(sizes)) {
    const opts = { ...size, theme };
    const tag = `${dev}-${theme}`;

    await run(`home-${tag}`, opts, async (page, shot) => {
      await page.goto(base + "/", { waitUntil: "networkidle" });
      await wait(2600);
      await shot("hero");
      await page.evaluate(() => document.getElementById("courses")?.scrollIntoView());
      await wait(1400);
      await shot("courses");
      await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
      await wait(1200);
      await shot("footer");
    });

    await run(`course-${tag}`, opts, async (page, shot) => {
      await page.goto(base + "/", { waitUntil: "networkidle" });
      await wait(900);
      await page.evaluate(() => document.getElementById("courses")?.scrollIntoView());
      await wait(900);
      await page.locator('a[href="/c/linear-algebra"]').first().click();
      await wait(250);
      await shot("morph");
      await wait(1200);
      await shot("page");
    });

    await run(`chapter-${tag}`, opts, async (page, shot) => {
      await page.goto(base + "/c/linear-algebra", { waitUntil: "networkidle" });
      await wait(700);
      await page.getByText("Matrices", { exact: true }).first().click();
      await wait(1600);
      await shot("grid");
      const card = page.locator('a[href*="/n/"]').first();
      await card.click();
      await wait(180);
      await shot("lightbox-morph");
      await wait(1300);
      await shot("lightbox");
      await page.keyboard.press("ArrowRight");
      await wait(700);
      await shot("lightbox-next");
      await page.keyboard.press("Escape");
      await wait(900);
      await shot("closed");
    });

    await run(`upload-${tag}`, opts, async (page, shot) => {
      await page.goto(base + "/c/linear-algebra", { waitUntil: "networkidle" });
      await wait(500);
      await page.getByText("Matrices", { exact: true }).first().click();
      await wait(1200);
      await page.keyboard.press("u");
      await wait(900);
      await shot("sheet");
      await page.locator('input[type="file"][multiple]').setInputFiles([
        "C:/Windows/Web/Screen/img100.jpg",
        "C:/Windows/Web/Screen/img101.jpg",
        "C:/Windows/Web/Wallpaper/ThemeA/img20.jpg",
      ]);
      await wait(900);
      await shot("files");
      await page.getByRole("button", { name: /Post note/ }).click();
      await wait(500);
      await shot("validation");
    });

    // Posts a real note (creates data), so only when asked: POST_NOTE=1
    if (process.env.POST_NOTE) await run(`post-${tag}`, opts, async (page, shot) => {
      await page.goto(base + "/c/linear-algebra", { waitUntil: "networkidle" });
      await wait(500);
      await page.getByText("Vectors", { exact: true }).first().click();
      await wait(1200);
      await page.keyboard.press("u");
      await wait(800);
      await page.locator('input[type="file"][multiple]').setInputFiles([
        "C:/Windows/Web/Screen/img103.jpg",
        "C:/Windows/Web/Wallpaper/ThemeC/img29.jpg",
      ]);
      await page.getByLabel("Title").fill(`Posted from the browser test (${tag})`);
      await page.getByLabel("Your name").fill("Playwright");
      await page.getByRole("button", { name: /Post note/ }).click();
      await wait(250);
      await shot("progress");
      await page.getByText("Posted", { exact: false }).first().waitFor({ timeout: 20000 });
      await wait(450);
      await shot("celebrate");
      await wait(1100);
      await shot("landed");
      await page.waitForURL(/\/n\//, { timeout: 5000 });
      await page.keyboard.press("Escape");
      await wait(1200);
      await shot("grid-after");
    });

    await run(`notfound-${tag}`, opts, async (page, shot) => {
      await page.goto(base + "/nope/nothing", { waitUntil: "networkidle" });
      await wait(1500);
      await shot("page");
    });
  }
}

await browser.close();
console.log(errors.length ? `ERRORS (${errors.length}):\n` + [...new Set(errors)].join("\n") : "no console errors");
