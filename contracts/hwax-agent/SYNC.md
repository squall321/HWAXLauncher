# Contracts sync notice

This directory is a **vendored copy** of the single source of truth that lives in
the HEAXHub repository at `contracts/hwax-agent/`. In the real workflow it is a
git **submodule** pinned to a release tag (`hwax-contracts-vX.Y.Z`); the files
here are a verbatim snapshot so the launcher can build offline.

| field | value |
|---|---|
| Synced from | `HEAXHub/contracts/hwax-agent/` |
| Contract version | **0.2.0** (see `CHANGELOG.md` / `VERSION`) |
| Synced at | 2026-06-06 |
| Consumed by | Rust HTTP client (`src-tauri`) + TypeScript types (`packages/schemas`) |

## ⚠ Stack note — the C#/WinUI3 comments upstream are stale

The upstream `openapi.yaml`, `tokens.css`, and `README.md` contain incidental
build comments that mention **"C# DTOs / `Heax.Agent.Api`" and a "WinUI3
launcher"**. Those are **stale** and do **not** apply to this repository.

HWAXLauncher (a.k.a. HWAX Agent) is **Tauri 2 + React 18 + TypeScript + Rust**,
per `docs/hwax-launcher-plan-v2.md` (the implementer's constitution) in the
HEAXHub repo. We consume these schemas from **Rust** (`jsonschema` crate) and
**TypeScript** (`ajv`), never from C#. The files are kept byte-for-byte
identical to upstream on purpose — do not "fix" the comments here; fix them
upstream so the source of truth stops contradicting the constitution.

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
