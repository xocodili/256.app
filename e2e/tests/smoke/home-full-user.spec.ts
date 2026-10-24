import { test, expect } from "@playwright/test";
import { loginAsDev, fullPersonaEmail, fullPersonaHandle, publicHandle } from "../../fixtures/auth";

test.describe("@m2 @smoke authenticated home", () => {
  test(
    "fully onboarded user sees feed shell",
    { tag: ["@journey-c", "@journey-d", "@us-home-01"] },
    async ({ page }) => {
      await loginAsDev(page, fullPersonaEmail());
      await expect(page).toHaveURL(/\/home/);

      const shell = page.locator(".app-shell");
      await expect(shell).toHaveAttribute("data-needs-profile", "0");
      await expect(page.locator("#app-main")).toBeVisible();
    },
  );

  test(
    "private profile page loads for self",
    { tag: ["@journey-c", "@us-home-02"] },
    async ({ page }) => {
      await loginAsDev(page, fullPersonaEmail());
      await page.goto(`/home/profile/${fullPersonaHandle()}`);
      await expect(page.locator(".app-shell")).toBeVisible();
      await expect(page.locator("#app-main")).toBeVisible();
    },
  );
});

test.describe("@m1 @smoke public profile", () => {
  test(
    "public entity page renders",
    { tag: ["@journey-b", "@us-home-03", "@us-pub-12"] },
    async ({ page }) => {
      await page.goto(`/public/${publicHandle()}`);
      await expect(page.locator("#entity-public-card")).toBeVisible();
    },
  );
});
