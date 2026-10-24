import { test, expect } from "@playwright/test";
import { publicHandle } from "../../fixtures/auth";

test.describe("@m1 @smoke public pages", () => {
  test(
    "root redirects to login",
    { tag: ["@journey-a", "@journey-e", "@us-pub-01"] },
    async ({ page }) => {
      await page.goto("/");
      await expect(page).toHaveURL(/\/login/);
      await expect(page.locator("#guest-split-auth")).toBeVisible();
    },
  );

  test(
    "login page loads guest marketing shell",
    { tag: ["@journey-a", "@journey-b", "@us-pub-02"] },
    async ({ page }) => {
      await page.goto("/login");
      await expect(page.locator("#guest-split-auth")).toBeVisible();
      await expect(page.locator("#auth-panel-join")).toBeVisible();
      await expect(page.locator("#auth-btn-join")).toBeVisible();
      await expect(page.locator("#auth-btn-login")).toBeVisible();
      await expect(page.locator("#auth-panel-constitution")).toHaveCount(0);
      await expect(page.locator(".public-footer")).toBeVisible();
      await expect(page.locator(".guest-welcome-heading")).toHaveText(
        "Socials done right",
      );
      await expect(page.locator(".guest-welcome-sub")).toContainText(
        "Discuss links and ideas",
      );
      const marketing = page.locator("#guest-welcome-marketing");
      await expect(marketing.locator(".guest-pantheon-card")).toBeVisible();
      await expect(marketing.locator(".guest-welcome-benefits")).toBeVisible();
      await expect(marketing.locator(".guest-benefit-tile")).toHaveCount(9);
      const pantheonTop = await marketing.locator(".guest-pantheon-card").boundingBox();
      const benefitTop = await marketing.locator(".guest-welcome-benefits").boundingBox();
      expect(pantheonTop).not.toBeNull();
      expect(benefitTop).not.toBeNull();
      if (pantheonTop && benefitTop) {
        expect(pantheonTop.y).toBeLessThan(benefitTop.y);
      }
      await expect(page.locator(".guest-pantheon-heading")).toHaveText(
        "Twelve AI stewards assist each community.",
      );
      await expect(page.locator(".guest-pantheon-lead")).toContainText(
        "safe by design",
      );
      await expect(page.locator(".guest-council-orbit a")).toHaveCount(12);
      await expect(page.locator("#auth-btn-join")).toHaveText("Create account");
    },
  );

  test(
    "join card opens register form and hides marketing",
    { tag: ["@journey-a", "@journey-b", "@us-pub-03"] },
    async ({ page }) => {
      await page.goto("/login");
      await page.locator("#auth-btn-join").click();
      await expect(page.locator("#auth-panel-register")).toBeVisible();
      await expect(page.locator('[data-auth-card="register"]')).toBeVisible();
      await expect(page.locator("#register-form")).toBeVisible();
      await expect(page.locator("#register-submit")).toBeVisible();
      await expect(page.locator("#guest-welcome-marketing")).toBeHidden();
    },
  );

  test(
    "login choice opens sign-in form",
    { tag: ["@journey-a", "@journey-c", "@us-pub-04"] },
    async ({ page }) => {
      await page.goto("/login");
      await page.locator("#auth-btn-login").click();
      await expect(page.locator("#auth-panel-login")).toBeVisible();
      await expect(page.locator("#login-form")).toBeVisible();
      await expect(page.locator("#email")).toBeVisible();
      await expect(page.locator("#password")).toBeVisible();
      await expect(page.locator("#guest-welcome-marketing")).toBeHidden();
    },
  );

  test(
    "login query param shows sign-in panel",
    { tag: ["@journey-c", "@journey-e", "@us-pub-05"] },
    async ({ page }) => {
      await page.goto("/login?login=1");
      await expect(page.locator("#auth-panel-login")).toBeVisible();
      await expect(page.locator("#auth-panel-join")).toBeHidden();
    },
  );

  test(
    "register redirect loads signup card",
    { tag: ["@journey-a", "@us-pub-06"] },
    async ({ page }) => {
      await page.goto("/register");
      await expect(page).toHaveURL(/\/login/);
      await expect(page.locator('[data-auth-card="register"]')).toBeVisible();
      await expect(page.locator("#register-form")).toBeVisible();
    },
  );

  test(
    "join query param loads signup card",
    { tag: ["@journey-b", "@us-pub-07"] },
    async ({ page }) => {
      await page.goto("/login?join=1");
      await expect(page.locator('[data-auth-card="register"]')).toBeVisible();
      await expect(page.locator("#register-form")).toBeVisible();
    },
  );

  test(
    "forgot password card loads from login panel",
    { tag: ["@journey-a", "@us-pub-08"] },
    async ({ page }) => {
      await page.goto("/login?login=1");
      await page.locator("#submit-forgot").click();
      await expect(page.locator("#auth-panel-forgot")).toBeVisible();
      await expect(page.locator('[data-auth-card="forgot"]')).toBeVisible();
    },
  );

  test(
    "reactivate account card loads from login panel",
    { tag: ["@journey-a", "@us-pub-09"] },
    async ({ page }) => {
      await page.goto("/login?login=1");
      await page.locator("#submit-reactivate").click();
      await expect(page.locator("#auth-panel-reactivate")).toBeVisible();
      await expect(page.locator('[data-auth-card="reactivate"]')).toBeVisible();
    },
  );

  test(
    "legal pages render via redirects",
    { tag: ["@journey-a", "@us-pub-10"] },
    async ({ page }) => {
      for (const [path, legal] of [
        ["/terms", "terms"],
        ["/privacy", "privacy"],
        ["/conditions", "terms"],
        ["/safety", "safety"],
        ["/community-rules", "constitution"],
      ] as const) {
        await page.goto(path);
        // Server redirects to /login?legal=…; login shell loads the card then strips the query.
        await expect(page).toHaveURL(/\/login(\?legal=.*)?$/);
        await expect(page.locator('[data-auth-card="legal"]')).toBeVisible();
      }
    },
  );

  test(
    "footer legal links open legal cards",
    { tag: ["@journey-a", "@us-pub-11"] },
    async ({ page }) => {
      await page.goto("/login");
      for (const doc of ["how", "safety", "constitution", "privacy", "terms"] as const) {
        await page.locator(`[data-auth-legal="${doc}"]`).click();
        await expect(page.locator('[data-auth-card="legal"]')).toBeVisible();
        await page.goto("/login");
      }
    },
  );

  test(
    "public entity page renders marketing shell and profile card",
    { tag: ["@journey-a", "@journey-b", "@us-pub-12"] },
    async ({ page }) => {
      await page.goto(`/public/${publicHandle()}`);
      const guestPanel = page.locator("#guest-split-public");
      await expect(guestPanel).toBeVisible();
      await expect(page.locator("#entity-public-card")).toBeVisible();
      await expect(guestPanel.locator(".guest-welcome-heading")).toHaveText(
        "Socials done right",
      );
      await expect(guestPanel.locator('a[href="/login?join=1"]')).toBeVisible();
      await expect(guestPanel.locator('a[href="/login?login=1"]')).toBeVisible();
      await expect(guestPanel.locator("#guest-public-benefits .guest-benefit-grid")).toBeVisible();
      await expect(guestPanel.locator(".guest-pantheon-card")).toBeVisible();
      await expect(guestPanel.locator(".guest-pantheon-heading")).toHaveText(
        "Twelve AI stewards assist each community.",
      );
      await expect(guestPanel.locator(".guest-council-orbit a")).toHaveCount(12);
      const viewport = page.viewportSize();
      const isDesktopEntityGrid =
        viewport !== null && viewport.width >= 900;
      const pantheonTop = await guestPanel.locator(".guest-pantheon-card").boundingBox();
      const profileTop = await page.locator("#entity-public-card").boundingBox();
      const benefitsTop = await guestPanel.locator("#guest-public-benefits").boundingBox();
      expect(pantheonTop).not.toBeNull();
      expect(profileTop).not.toBeNull();
      expect(benefitsTop).not.toBeNull();
      if (pantheonTop && profileTop && benefitsTop) {
        if (isDesktopEntityGrid) {
          const stackTop = await guestPanel
            .locator(".guest-public-stack-top")
            .boundingBox();
          const joinTop = await guestPanel.locator(".guest-join-card").boundingBox();
          expect(stackTop).not.toBeNull();
          expect(joinTop).not.toBeNull();
          if (stackTop && joinTop) {
            expect(joinTop.y).toBeLessThan(pantheonTop.y);
            expect(profileTop.x).toBeGreaterThan(stackTop.x);
            const rowBottom = Math.max(
              stackTop.y + stackTop.height,
              profileTop.y + profileTop.height,
            );
            expect(benefitsTop.y).toBeGreaterThan(rowBottom - 1);
          }
        } else {
          expect(pantheonTop.y).toBeLessThan(profileTop.y);
          expect(profileTop.y).toBeLessThan(benefitsTop.y);
        }
      }
    },
  );

  test(
    "brand stylesheet loads",
    { tag: ["@journey-a", "@us-pub-13"] },
    async ({ page }) => {
      const res = await page.request.get("/static/css/brand.css");
      expect(res.ok()).toBeTruthy();
      expect(res.headers()["content-type"] ?? "").toMatch(/text\/css/);
    },
  );

  test(
    "health status responds",
    { tag: ["@journey-a", "@us-pub-14"] },
    async ({ request }) => {
      const res = await request.get("/health/status");
      expect(res.ok()).toBeTruthy();
    },
  );
});
