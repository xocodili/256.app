import { existsSync, readFileSync } from "fs";
import { join } from "path";

export type PersonaEntry = {
  email: string;
  handle?: string;
  user_id: number;
};

export type E2eManifest = {
  base_password: string;
  terms_consent_version: string;
  personas: {
    full: PersonaEntry;
    consent: PersonaEntry;
    partial: PersonaEntry;
  };
  fixtures: {
    public_handle: string;
    sample_post_body: string;
  };
};

const manifestPath = join(__dirname, "manifest.json");
const examplePath = join(__dirname, "manifest.example.json");

export function loadManifest(): E2eManifest {
  const path = existsSync(manifestPath) ? manifestPath : examplePath;
  if (!existsSync(path)) {
    throw new Error(
      "Missing e2e/fixtures/manifest.json — run: cargo run --bin e2e_seed -- --manifest e2e/fixtures/manifest.json",
    );
  }
  return JSON.parse(readFileSync(path, "utf8")) as E2eManifest;
}

export const manifest = loadManifest();
