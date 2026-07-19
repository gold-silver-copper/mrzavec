import { expect, test } from "@playwright/test";

const SAVE_KEY = "mrzavec.save.v12:default";
const ACTIVE_SAVE_KEY = "mrzavec.save.active";
const SCORE_KEY = "mrzavec.scores.v1:local";

async function waitForGame(page, canvas) {
  await expect(canvas).toBeVisible();
  await expect
    .poll(() => canvas.evaluate((element) => element.width))
    .toBeGreaterThanOrEqual(824);
  await expect
    .poll(() => canvas.evaluate((element) => element.height))
    .toBeGreaterThanOrEqual(480);
  await expect
    .poll(() => page.evaluate(() => document.activeElement?.id))
    .toBe("mrzavec");
}

async function openGame(page) {
  const errors = [];
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  await page.goto("/web/");
  const canvas = page.locator("#mrzavec");
  await waitForGame(page, canvas);
  return { canvas, errors };
}

async function expectCanvasChange(canvas, action) {
  const before = await canvas.screenshot();
  await action();
  await expect
    .poll(async () => Buffer.compare(before, await canvas.screenshot()), {
      timeout: 30_000,
    })
    .not.toBe(0);
}

test("mounts, focuses, handles keyboard commands, and scales responsively", async ({
  page,
}) => {
  const { canvas, errors } = await openGame(page);
  const initial = await canvas.screenshot();

  await page.keyboard.press("Shift+/");
  await page.keyboard.press("Shift+8");
  await expect
    .poll(async () => Buffer.compare(initial, await canvas.screenshot()))
    .not.toBe(0);

  await page.keyboard.press("Space");
  await page.keyboard.press("h");
  await page.keyboard.press("j");
  await page.keyboard.press("Control+p");
  await page.keyboard.press("Control+r");
  await expect(canvas).toBeVisible();

  await page.setViewportSize({ width: 500, height: 700 });
  const box = await canvas.boundingBox();
  expect(box).not.toBeNull();
  expect(box.width).toBeLessThanOrEqual(500);
  expect(Math.abs(box.width / box.height - 824 / 480)).toBeLessThan(0.02);
  expect(errors).toEqual([]);
});

test("keyboard command mappings open their game views", async ({
  browserName,
  page,
}) => {
  test.skip(browserName !== "chromium", "the full mapping check runs once in Chrome");
  const { canvas, errors } = await openGame(page);

  await expectCanvasChange(canvas, () => page.keyboard.press("Shift+1"));
  for (const key of ["q", "r", "e", "w", "d", "t", "z", "i", "o", "f"]) {
    await test.step(`maps the ${key} command`, async () => {
      await expectCanvasChange(canvas, () => page.keyboard.press(key));
      if (key === "i") {
        await page.keyboard.press("Space");
      } else {
        await page.keyboard.press("Escape");
        if (key === "o") await page.keyboard.press("Space");
      }
    });
  }
  expect(errors).toEqual([]);
});

test("a keyboard turn reaches the simulation and persisted game state", async ({
  page,
}) => {
  const { canvas, errors } = await openGame(page);
  await page.keyboard.press("Shift+S");
  await page.keyboard.press("y");
  await expect
    .poll(() => page.evaluate((key) => localStorage.getItem(key), SAVE_KEY))
    .not.toBeNull();
  const initialTurn = await page.evaluate(
    (key) => JSON.parse(localStorage.getItem(key)).turn,
    SAVE_KEY,
  );

  await page.reload();
  await waitForGame(page, canvas);
  await page.keyboard.press(".");
  await page.keyboard.press("Shift+S");
  await page.keyboard.press("y");
  await expect
    .poll(() =>
      page.evaluate(
        ([key, turn]) => JSON.parse(localStorage.getItem(key) ?? "null")?.turn > turn,
        [SAVE_KEY, initialTurn],
      ),
    )
    .toBe(true);
  expect(errors).toEqual([]);
});

test("the active logical save slot is restored exactly once", async ({ page }) => {
  const { canvas, errors } = await openGame(page);
  await page.keyboard.press("Shift+S");
  await page.keyboard.press("y");
  await expect
    .poll(() => page.evaluate((key) => localStorage.getItem(key), SAVE_KEY))
    .not.toBeNull();
  expect(await page.evaluate((key) => localStorage.getItem(key), ACTIVE_SAVE_KEY)).toBe(
    "default",
  );

  await page.evaluate(
    ([saveKey, activeKey]) => {
      const saved = localStorage.getItem(saveKey);
      const game = JSON.parse(saved);
      game.player.gold = 42;
      localStorage.setItem("mrzavec.save.v12:campaign", JSON.stringify(game));
      localStorage.removeItem(saveKey);
      localStorage.setItem(activeKey, "campaign");
    },
    [SAVE_KEY, ACTIVE_SAVE_KEY],
  );

  await page.reload();
  await waitForGame(page, canvas);
  await expect
    .poll(() =>
      page.evaluate(() => localStorage.getItem("mrzavec.save.v12:campaign")),
    )
    .toBeNull();

  const restored = await canvas.screenshot();
  await page.keyboard.press("Shift+Q");
  await page.keyboard.press("y");
  await expect
    .poll(() => page.evaluate((key) => localStorage.getItem(key), SCORE_KEY))
    .not.toBeNull();
  await expect
    .poll(async () => Buffer.compare(restored, await canvas.screenshot()))
    .not.toBe(0);
  expect(
    await page.evaluate(
      (key) => JSON.parse(localStorage.getItem(key))[0].score,
      SCORE_KEY,
    ),
  ).toBe(42);
  expect(errors).toEqual([]);
});

test("quota failure is visible and does not stop or create a save", async ({ page }) => {
  await page.addInitScript(() => {
    Storage.prototype.setItem = () => {
      throw new DOMException("test quota exhausted", "QuotaExceededError");
    };
  });
  const { canvas, errors } = await openGame(page);
  const prompt = await canvas.screenshot();
  await page.keyboard.press("Shift+S");
  await page.keyboard.press("y");
  await expect
    .poll(async () => Buffer.compare(prompt, await canvas.screenshot()))
    .not.toBe(0);
  await expect(canvas).toBeVisible();
  expect(await page.evaluate((key) => localStorage.getItem(key), SAVE_KEY)).toBeNull();
  expect(errors).toEqual([]);
});

test("a corrupt save remains available and the game starts safely", async ({ page }) => {
  await page.addInitScript(
    ({ key, value }) => localStorage.setItem(key, value),
    { key: SAVE_KEY, value: "not valid json" },
  );
  const { canvas, errors } = await openGame(page);
  await expect(canvas).toBeVisible();
  expect(await page.evaluate((key) => localStorage.getItem(key), SAVE_KEY)).toBe(
    "not valid json",
  );
  expect(errors).toEqual([]);
});

test("a corrupt active slot falls back to a valid default save", async ({ page }) => {
  const { canvas, errors } = await openGame(page);
  await page.keyboard.press("Shift+S");
  await page.keyboard.press("y");
  await expect
    .poll(() => page.evaluate((key) => localStorage.getItem(key), SAVE_KEY))
    .not.toBeNull();

  await page.evaluate(
    ([saveKey, activeKey]) => {
      const fallback = JSON.parse(localStorage.getItem(saveKey));
      fallback.player.gold = 777;
      localStorage.setItem(saveKey, JSON.stringify(fallback));
      localStorage.setItem("mrzavec.save.v12:campaign", "not valid json");
      localStorage.setItem(activeKey, "campaign");
    },
    [SAVE_KEY, ACTIVE_SAVE_KEY],
  );

  await page.reload();
  await waitForGame(page, canvas);
  await expect
    .poll(() => page.evaluate((key) => localStorage.getItem(key), SAVE_KEY))
    .toBeNull();
  expect(await page.evaluate((key) => localStorage.getItem(key), ACTIVE_SAVE_KEY)).toBeNull();
  expect(
    await page.evaluate(() => localStorage.getItem("mrzavec.save.v12:campaign")),
  ).toBe("not valid json");

  await page.keyboard.press("Shift+Q");
  await page.keyboard.press("y");
  await expect
    .poll(() => page.evaluate((key) => localStorage.getItem(key), SCORE_KEY))
    .not.toBeNull();
  expect(
    await page.evaluate(
      (key) => JSON.parse(localStorage.getItem(key))[0].score,
      SCORE_KEY,
    ),
  ).toBe(777);
  expect(errors).toEqual([]);
});

test("disabled storage starts safely", async ({ page }) => {
  await page.addInitScript(() => {
    Storage.prototype.getItem = () => {
      throw new DOMException("test storage disabled", "SecurityError");
    };
  });
  const { canvas, errors } = await openGame(page);
  await expect(canvas).toBeVisible();
  expect(errors).toEqual([]);
});

test("high-DPI rendering produces full-resolution browser output", async ({ page }) => {
  const { canvas, errors } = await openGame(page);
  const ratio = await page.evaluate(() => window.devicePixelRatio);
  const box = await canvas.boundingBox();
  const screenshot = await canvas.screenshot();
  expect(ratio).toBeGreaterThan(1);
  expect(box).not.toBeNull();
  expect(screenshot.readUInt32BE(16)).toBeGreaterThanOrEqual(
    Math.floor(box.width * ratio),
  );
  expect(screenshot.readUInt32BE(20)).toBeGreaterThanOrEqual(
    Math.floor(box.height * ratio),
  );
  expect(errors).toEqual([]);
});
