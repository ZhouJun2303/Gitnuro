import { expect, test } from "@playwright/test";

for (const size of [{ width: 240, height: 240 }, { width: 300, height: 620 }, { width: 500, height: 480 }, { width: 760, height: 480 }]) {
  test(`conflict pane stays readable and selectable at ${size.width}px`, async ({ page }) => {
    test.setTimeout(60_000);
    await page.goto("/?scene=conflict-compact");
    await expect(page.locator('[data-lab-scene="conflict-compact"]')).toBeVisible();
    const pane = page.locator(".conflict-pane");
    await expect(pane.locator(".conflict-tool-group")).toBeVisible();
    const original = (await pane.boundingBox())!;
    const viewport = page.viewportSize()!;
    await page.setViewportSize({ width: Math.round(viewport.width + size.width - original.width), height: Math.round(viewport.height + size.height - original.height) });
    await expect.poll(async () => Math.round((await pane.boundingBox())!.width)).toBe(size.width);
    await expect(pane.locator(".conflict-title")).toBeInViewport();

    const layout = await pane.evaluate((node) => {
      const bounds = node.getBoundingClientRect();
      const cards = [...node.querySelectorAll<HTMLElement>(".conflict-card")].map((card) => card.getBoundingClientRect());
      const checks = [...node.querySelectorAll<HTMLInputElement>(".conflict-bridge input")].map((input) => input.getBoundingClientRect());
      const elements = [...node.querySelectorAll<HTMLElement>(".conflict-card, .conflict-merge, .conflict-tool-group, .conflict-tool")];
      return {
        overflow: node.scrollWidth - node.clientWidth,
        outside: elements.some((element) => { const box = element.getBoundingClientRect(); return box.left < bounds.left || box.right > bounds.right + 1; }),
        cards: cards.map((box) => ({ x: box.x, y: box.y, bottom: box.bottom })),
        aligned: cards.every((card, index) => Math.abs((card.top + card.bottom - checks[index].top - checks[index].bottom) / 2) < 1),
        namesClipped: [...node.querySelectorAll<HTMLElement>(".conflict-card .name")].some((name) => name.scrollWidth > name.clientWidth),
      };
    });
    expect(layout.overflow).toBeLessThanOrEqual(1);
    expect(layout.outside).toBe(false);
    expect(layout.namesClipped).toBe(false);
    if (size.width <= 520) {
      expect(layout.cards[1].y).toBeGreaterThanOrEqual(layout.cards[0].bottom);
      expect(layout.aligned).toBe(true);
    } else {
      expect(layout.cards[0].y).toBe(layout.cards[1].y);
      expect(layout.cards[1].x).toBeGreaterThan(layout.cards[0].x);
    }

    const cards = pane.locator(".conflict-card");
    await cards.first().click();
    await expect(pane.locator("#conflict-theirs")).not.toBeChecked();
    await expect(pane.locator("#conflict-ours")).toBeChecked();
    await expect(pane.locator(".conflict-tool-group")).toHaveCount(0);
    await cards.last().click();
    await expect(pane.locator(".conflict-merge")).toBeDisabled();
    await cards.first().click();
    await cards.last().click();
    await expect(pane.locator(".conflict-merge")).toBeEnabled();
    await pane.locator(".conflict-tool-group").scrollIntoViewIfNeeded();
    await expect(pane.locator(".conflict-tool-group")).toBeInViewport();
    await pane.evaluate((node) => { node.scrollTop = 0; });
    await expect(pane.locator(".conflict-title")).toBeInViewport();
    await pane.screenshot({ path: `.lab/conflict-pane-${size.width}.png` });
  });
}
