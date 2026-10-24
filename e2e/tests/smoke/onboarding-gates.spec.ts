import { test, expect } from "@playwright/test";
import {
  loginAsDev,
  consentPersonaEmail,
  partialPersonaEmail,
} from "../../fixtures/auth";

test.describe("@m2 @smoke onboarding gates", () => {
  test(
    "consent-only user is routed to terms step",
    { tag: ["@journey-d", "@us-onb-01"] },
    async ({ page }) => {
      await loginAsDev(page, consentPersonaEmail());
      await page.goto("/home");
      await expect(page).toHaveURL(/onboarding=consent/);
    },
  );

  test(
    "partial user is routed to face capture onboarding",
    { tag: ["@journey-d", "@us-onb-02"] },
    async ({ page }) => {
      await loginAsDev(page, partialPersonaEmail());
      await page.goto("/home");
      await expect(page).toHaveURL(/\/onboarding/);
    },
  );
});
