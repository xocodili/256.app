import { test, expect } from "@playwright/test";
import {
  loginViaForm,
  fullPersonaEmail,
  basePassword,
} from "../../fixtures/auth";

test.describe("@m1 @regression auth", () => {
  test(
    "form login reaches home for full persona",
    { tag: ["@journey-c", "@us-auth-01"] },
    async ({ page }) => {
      await loginViaForm(page, fullPersonaEmail(), basePassword());
      await expect(page).toHaveURL(/\/home/);
      await expect(page.locator(".app-shell")).toHaveAttribute(
        "data-needs-profile",
        "0",
      );
    },
  );
});
