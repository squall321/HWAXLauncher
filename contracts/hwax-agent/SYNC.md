# Contracts sync notice

This directory is a **vendored copy** of the single source of truth that lives in
the HEAXHub repository at `contracts/hwax-agent/`. In the real workflow it is a
git **submodule** pinned to a release tag (`hwax-contracts-vX.Y.Z`); the files
here are a verbatim snapshot so the launcher can build offline.

| field | value |
|---|---|
| Synced from | `HEAXHub/contracts/hwax-agent/` |
| Contract version | **0.2.0 + Unreleased** (post-0.2.0 feedback merged via squall321/HEAXHub#1) |
| Synced at | 2026-06-06 (re-synced after PR #1 merge) |
| Consumed by | Rust HTTP client (`src-tauri`) + TypeScript types (`packages/schemas`) |

## Stack note — C#/WinUI3 references removed upstream (resolved)

Earlier upstream `openapi.yaml`, `tokens.css`, and `README.md` carried incidental
build comments mentioning **"C# DTOs / `Heax.Agent.Api`" and a "WinUI3 launcher"**
— stale wording that contradicted the confirmed stack and actually misled an
HWAXAgent-side LLM into starting a WinUI3/.NET build. Those were **fixed
upstream in [squall321/HEAXHub#1](https://github.com/squall321/HEAXHub/pull/1)**;
this synced copy is the **corrected** contract (no C#/WinUI3 wording; the
`install-report` description uses the right `/api/v1/launcher-agents/installs`
prefix; `openapi.yaml` now documents the Tauri updater feed
`GET /api/v1/installers/{app_id}/latest`).

HWAXLauncher (a.k.a. HWAX Agent) is **Tauri 2 + React 18 + TypeScript + Rust**,
per `docs/hwax-launcher-plan-v2.md` (the implementer's constitution). We consume
these schemas from **Rust** (`jsonschema` crate) and **TypeScript** (`ajv`),
never from C#. Keep this copy byte-for-byte identical to upstream — any further
contract change goes through a HEAXHub PR (split-strategy §6.1), not a local edit.

## What is authoritative here

Only the **wire format** is authoritative:

- `manifest.schema.json` — `GET /api/v1/launcher-agents/manifest` response.
- `install-report.schema.json` — `POST /api/v1/launcher-agents/installs` body.
- `audit-event.schema.json` — `POST /api/v1/launcher-agents/audit` body.
- `openapi.yaml` — endpoint paths, auth, enroll/refresh/heartbeat shapes.
- `tokens.css` — design tokens (dark + amber) the React UI mirrors.

`additionalProperties: false` is set on every object — the agent must never send
a key that is not in the schema.

## Re-syncing

```sh
pnpm fetch-schemas            # scripts/fetch-schemas.mjs — pulls a pinned tag
```
