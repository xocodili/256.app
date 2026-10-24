import type { Page } from "@playwright/test";
import { manifest } from "./manifest";

export async function loginAsDev(page: Page, email: string, next = "/home") {
  const q = new URLSearchParams({ email, next });
  await page.goto(`/dev/auth/login-as?${q.toString()}`);
}

export async function loginViaForm(page: Page, email: string, password: string) {
  await page.goto("/login?login=1");
  await page.locator("#auth-panel-login").waitFor({ state: "visible" });
  await page.locator("#email").fill(email);
  await page.locator("#password").fill(password);
  await page.locator("#login-form").getByRole("button", { name: "Sign in" }).click();
}

export function fullPersonaEmail() {
  return manifest.personas.full.email;
}

export function fullPersonaHandle() {
  return manifest.personas.full.handle ?? "e2efull";
}

export function consentPersonaEmail() {
  return manifest.personas.consent.email;
}

export function partialPersonaEmail() {
  return manifest.personas.partial.email;
}

export function basePassword() {
  return manifest.base_password;
}

export function publicHandle() {
  return manifest.fixtures.public_handle;
}

export function samplePostBody() {
  return manifest.fixtures.sample_post_body;
}
