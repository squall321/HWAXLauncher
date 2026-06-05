#!/usr/bin/env node
// scripts/fetch-schemas.mjs
//
// Sync contracts/hwax-agent/ from the HEAXHub repo (the single source of truth)
// at a PINNED tag, or verify the vendored copy is unmodified.
//
// Two modes:
//   node scripts/fetch-schemas.mjs            → fetch the pinned tag, write files
//   node scripts/fetch-schemas.mjs --check    → no network; verify the vendored
//                                               snapshot is intact (used in CI)
//
// Why this exists (docs/CONTRIBUTING.md §2, split-strategy §4/§6):
//   contracts/hwax-agent/ is OWNED by the HEAXHub repo. Here it is a *vendored
//   snapshot* pinned to a release tag. In the real workflow it is a git
//   **submodule** at the same path:
//
//       git submodule add -b main \
//         https://github.com/squall321/HEAXHub.git contracts/_heaxhub-upstream
//       # then symlink/copy contracts/_heaxhub-upstream/contracts/hwax-agent
//       # → contracts/hwax-agent, pinned with:
//       git -C contracts/_heaxhub-upstream checkout hwax-contracts-v0.2.0
//
//   This script is the no-submodule fallback: it pulls the same files over HTTPS
//   from raw.githubusercontent at the pinned tag so the launcher can build
//   offline once vendored. NEVER hand-edit the vendored files — re-run this.
//
// No secrets: public raw files only. If the upstream repo is private, set
// GITHUB_TOKEN in the environment and it is sent as a bearer header.

import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile, readdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, "..");

// ── Pin point. Bump this (and re-run) when adopting a new contract version. ──
const UPSTREAM_OWNER = "squall321";
const UPSTREAM_REPO = "HEAXHub";
const UPSTREAM_TAG = "hwax-contracts-v0.2.0"; // pinned release tag
const UPSTREAM_DIR = "contracts/hwax-agent"; // path within the HEAXHub repo

// Files that make up the contract surface (the authoritative wire format).
const FILES = [
  "VERSION",
  "CHANGELOG.md",
  "README.md",
  "openapi.yaml",
  "manifest.schema.json",
  "install-report.schema.json",
  "audit-event.schema.json",
  "tokens.css",
];

const LOCAL_DIR = join(REPO_ROOT, "contracts", "hwax-agent");

const rawUrl = (file) =>
  `https://raw.githubusercontent.com/${UPSTREAM_OWNER}/${UPSTREAM_REPO}/${UPSTREAM_TAG}/${UPSTREAM_DIR}/${file}`;

const sha256 = (buf) => createHash("sha256").update(buf).digest("hex");

function log(msg) {
  process.stdout.write(`[fetch-schemas] ${msg}\n`);
}

async function fetchFile(file) {
  const url = rawUrl(file);
  const headers = { "User-Agent": "hwax-fetch-schemas" };
  if (process.env.GITHUB_TOKEN) {
    headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  }
  const res = await fetch(url, { headers });
  if (!res.ok) {
    throw new Error(`GET ${url} → ${res.status} ${res.statusText}`);
  }
  return Buffer.from(await res.arrayBuffer());
}

// --check: verify the vendored snapshot exists and is internally consistent.
// We do NOT hit the network here (CI runs it on every PR, including offline
// forks). We assert: every expected file is present, VERSION matches the tag,
// and (cheap structural) the JSON schemas parse. This catches an accidental
// hand-edit that breaks the files; the authoritative drift gate is the Rust
// schema-conformance test.
async function check() {
  if (!existsSync(LOCAL_DIR)) {
    throw new Error(`vendored contracts missing: ${LOCAL_DIR}`);
  }
  const present = new Set(await readdir(LOCAL_DIR));
  const missing = FILES.filter((f) => !present.has(f));
  if (missing.length) {
    throw new Error(`vendored contracts incomplete, missing: ${missing.join(", ")}`);
  }

  const version = (await readFile(join(LOCAL_DIR, "VERSION"), "utf8")).trim();
  const tagVersion = UPSTREAM_TAG.replace(/^hwax-contracts-v/, "");
  if (version !== tagVersion) {
    throw new Error(
      `VERSION (${version}) does not match the pinned tag (${tagVersion}). ` +
        `Re-run \`node scripts/fetch-schemas.mjs\` after bumping UPSTREAM_TAG.`,
    );
  }

  for (const f of FILES.filter((f) => f.endsWith(".json"))) {
    try {
      JSON.parse(await readFile(join(LOCAL_DIR, f), "utf8"));
    } catch (e) {
      throw new Error(`vendored ${f} is not valid JSON: ${e.message}`);
    }
  }

  log(`OK — vendored contracts v${version} present and well-formed (tag ${UPSTREAM_TAG}).`);
}

// Default mode: fetch each file at the pinned tag and write it, reporting a
// per-file sha256 so a reviewer can confirm what changed.
async function fetchAll() {
  await mkdir(LOCAL_DIR, { recursive: true });
  log(`syncing ${UPSTREAM_OWNER}/${UPSTREAM_REPO}@${UPSTREAM_TAG}:${UPSTREAM_DIR} → ${LOCAL_DIR}`);

  for (const file of FILES) {
    const buf = await fetchFile(file);
    const dest = join(LOCAL_DIR, file);
    await mkdir(dirname(dest), { recursive: true });
    await writeFile(dest, buf);
    log(`  ${file}  (sha256 ${sha256(buf).slice(0, 16)}…)`);
  }

  const version = (await readFile(join(LOCAL_DIR, "VERSION"), "utf8")).trim();
  log(`done — contracts now at v${version}. Commit the change and update SYNC.md.`);
}

async function main() {
  const checkMode = process.argv.includes("--check");
  try {
    if (checkMode) {
      await check();
    } else {
      await fetchAll();
    }
  } catch (err) {
    process.stderr.write(`[fetch-schemas] ERROR: ${err.message}\n`);
    process.exit(1);
  }
}

main();
