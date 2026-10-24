import type { FullConfig } from "@playwright/test";

const baseURL = process.env.BASE_URL ?? "http://127.0.0.1:8080";
const healthPath = "/health/status";
const timeoutMs = 60_000;
const intervalMs = 500;

export default async function globalSetup(_config: FullConfig) {
  const deadline = Date.now() + timeoutMs;
  const url = `${baseURL.replace(/\/$/, "")}${healthPath}`;

  while (Date.now() < deadline) {
    try {
      const res = await fetch(url);
      if (res.ok) {
        return;
      }
    } catch {
      // server not up yet
    }
    await new Promise((r) => setTimeout(r, intervalMs));
  }

  throw new Error(
    `App did not respond OK at ${url} within ${timeoutMs / 1000}s — start cargo run first`,
  );
}
