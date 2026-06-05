# HWAX Agent — Architecture

> Single source of truth: HEAXHub `docs/hwax-launcher-plan-v2.md` (the implementer's
> constitution). This file summarizes the parts an implementer must hold in their
> head, with pointers back to the plan sections. Where the plan and this file
> disagree, the plan wins.
>
> **Stack is fixed:** Tauri 2 (Rust core) + React 18 + TypeScript + Vite + Tailwind.
> WinUI3 / WPF / .NET / C# / XAML / MAUI / Avalonia / Electron / Flutter are
> forbidden (see `docs/adr/0001-tauri-2-over-winui3.md`).

---

## 1. The HEAXHub ↔ Agent split

HEAXHub (the server) and HWAX Agent (the Windows client) communicate over exactly
**one surface**: the HTTP contract vendored at `contracts/hwax-agent/`
(`openapi.yaml` + the three `*.schema.json` files + `tokens.css`). Nothing else
crosses the boundary.

```
HEAXHub Server (Linux, FastAPI)                User PC (Windows 10/11)
┌──────────────────────────────────┐          ┌─────────────────────────────────┐
│ /api/v1/launcher-agents/enroll    │          │ HWAX Agent (Tauri 2, single      │
│ /api/v1/launcher-agents/refresh   │          │ per-user process, asInvoker)     │
│ /api/v1/launcher-agents/manifest  │◄──HTTPS──┤  ├─ React 18 UI (tray panel,    │
│ /api/v1/launcher-agents/installs  │  bearer  │  │   settings, module list)      │
│ /api/v1/launcher-agents/audit     │   JWT    │  ├─ src-tauri (thin shell)       │
│ /api/v1/launcher-agents/heartbeat │          │  │   reqwest · keyring · tray    │
│ /api/v1/installers/{id}/download  │          │  │   · process spawn             │
│                                   │          │  └─ hwax-core (pure logic)       │
│ Postgres · object storage (S3)    │          │      verify · zip-safe · swap    │
└──────────────────────────────────┘          │      · rollback · GC · state     │
                                               │ %LocalAppData%\HWAXAgent\        │
                                               └─────────────────────────────────┘
```

**The asymmetry (plan-v2 §3):** HEAXHub owns the *catalog, auth, storage, and audit
log*. The Agent owns *tray + download + verify + swap + execute*. The Agent never
exposes a free-form URL field, never runs an arbitrary exe, never asks for admin.

### Responsibilities

| HEAXHub | HWAX Agent |
|---|---|
| Issue `enrollment_token` (operator, single-use) | Exchange it for a device JWT pair, store in Credential Manager |
| Serve `manifest` (programs.json) with ETag | Diff against local `current.json`, decide install/update |
| Issue presigned installer download URLs + record SHA-256 | Verify SHA-256 before extract/execute |
| Receive install reports + audit events | Emit reports/audit on install/fail/rollback/AV-suspect |
| Catalog / approval / storage / audit retention | Tray UX, version GC, run, rollback |

### Endpoint prefix — read this once

Launcher endpoints live under **`/api/v1/launcher-agents/*`**, *not*
`/api/v1/agents/*`. The bare `/api/v1/agents/*` prefix is the **pre-existing
service-agent (polling) API** with a different body shape; registering launcher
routes there would double-register. This rename is contract **v0.2.0 BREAKING**
(`contracts/hwax-agent/CHANGELOG.md`). The single exception is
`GET /api/v1/installers/{id}/download`, which is shared and unchanged.

---

## 2. The hwax-core ↔ src-tauri layering

The defining structural choice (README "왜 crates/hwax-core 를 분리했는가"): the
correctness-critical logic is a **pure crate with no Tauri / WebView2 dependency**,
so it can be exercised by `cargo test -p hwax-core` on any machine (including the
Linux CI lane / a dev box without WebView2).

```
┌────────────────────────────────────────────────────────────┐
│ apps/agent/src/            React 18 + TypeScript UI          │
│   ipc/  → typed invoke() wrappers (single IPC surface)       │
└───────────────┬────────────────────────────────────────────┘
                │ Tauri IPC (invoke / emit-listen events)
┌───────────────▼────────────────────────────────────────────┐
│ apps/agent/src-tauri/      THE SHELL — thin adapters only    │
│   commands/  tray/  auth/  sync/  telemetry/  download/      │
│   Supplies the IO hwax-core deliberately does NOT do:        │
│     • reqwest      async HTTPS download (+ bearer JWT)        │
│     • keyring      Windows Credential Manager (JWT/refresh)  │
│     • tray / IPC   menu, status dot, toasts                  │
│     • process      spawn post_install_check & run_module     │
└───────────────┬────────────────────────────────────────────┘
                │ plain function calls (no async, no IO ambient)
┌───────────────▼────────────────────────────────────────────┐
│ crates/hwax-core/          PURE LOGIC — already built/tested │
│   hash       sha256_file / verify_file / digests_match       │
│   zip_safe   extract_zip_safe / entry_escapes (zip-slip)     │
│   atomic     write_atomic / write_atomic_json (rename)       │
│   install    perform_swap / rollback / gc_old_versions       │
│   origin     origin_of / is_allowed / ensure_allowed         │
│   store      CurrentJson / InstallMeta read/write            │
│   state      ModuleState (14) / InstallPhase / decide_state  │
│   manifest   Manifest/Program/Package/Entry/Lifecycle (recv) │
│   report     InstallReport / InstallStatus (sent)            │
│   audit      AuditEvent / AuditKind / ClientMeta (sent)      │
│   config     AgentConfig (+ effective_allowed_origins)       │
└─────────────────────────────────────────────────────────────┘
```

**The rule: `hwax-core` does no IO that touches the network, the credential store,
or process spawning, and it never depends on Tauri.** It owns the bytes-in /
bytes-out and filesystem-rename logic. The shell owns everything async and
platform. This is why the security-critical paths are unit-tested without a GUI.

### Contract fidelity (hwax-core lib.rs)
- **Received** types (`manifest`) tolerate unknown fields → forward-compatible
  (PR-protocol §3 graceful degradation).
- **Sent** types (`install-report`, `audit-event`) serialize *only* schema keys,
  because every contract object is `additionalProperties: false`.
- `crates/hwax-core/tests/schema_conformance.rs` validates built payloads against
  the **real** `contracts/hwax-agent/*.schema.json` (Draft 2020-12). If a type and
  its schema drift, that test fails — this is the repo's contract gate
  (`.github/workflows/contracts-validate.yml`).

---

## 3. Module lifecycle state machine (plan-v2 §6)

`hwax_core::state::ModuleState` — 14 variants, serialized to the UI as snake_case.

```
        ┌──────┐  tick / click      ┌──────────┐
        │ idle │ ─────────────────► │ checking │
        └──────┘ ◄────────┐         └────┬─────┘
                          │   ┌──────────┼───────────┐
                          │   │ same     │ newer     │ missing
                          │   ▼          ▼           ▼
                          │ installed  outdated   not_installed
                          │   │          │           │
                          │   │ run      │ update     │ install
                          │   ▼          ▼            ▼
                          │ running   downloading ◄───┘
                          │   │ exit     │
                          │   ▼          ▼
                          │ stopped   verifying  ── sha256 ok ─►
                          │              │
                          │              ▼
                          │          extracting
                          │              │ post_install_check ok
                          │              ▼
                          │           swapping (atomic)
                          │              │
                          └──── installed ◄┘

  Failure branch (verifying / extracting / post_install_check fail):
        failed ──manual──► rolling_back ──► rolled_back ──► idle
```

State derivation is pure: `decide_state(local: Option<&str>, latest)` returns
`NotInstalled` / `Outdated` / `Installed` via SemVer comparison (`is_newer`).
Install progress is streamed separately through the `install:progress` Tauri event
carrying `InstallPhase` (`download | verify | extract | check | swap`) + percent.

### The install / swap algorithm (plan-v2 §8, implemented in hwax-core)
1. Download to `cache/downloads/{id}-{ver}.zip.partial` — **only** if
   `ensure_allowed(url, allowed_origins)` passes (exact origin match).
2. `verify_file(partial, manifest.sha256)` — mismatch ⇒ delete + `sha256_mismatch`
   audit + abort.
3. `extract_zip_safe(partial, "{ver}.staging")` — each entry canonicalized; any
   entry escaping the destination aborts (zip-slip defense).
4. Run `post_install_check` (shell spawns it); failure ⇒ `rm` staging, leave
   `current.json` untouched, audit, abort.
5. `perform_swap`: rename `{ver}.staging` → `{ver}` (same-volume atomic), then
   `write_atomic_json(current.json, …)` (temp + rename).
6. `gc_old_versions(keep_last_n)` — current + previous always preserved.

### Rollback (plan-v2 §9)
**Only `current.json` is swapped; version directories are never deleted on
rollback.** `rollback(...)` points `current.json` back at `previous_version` (must
still be on disk; GC'd ⇒ error), records `rolled_back_from`, and emits a
`rolled_back` audit event.

---

## 4. Local folder layout — `%LocalAppData%\HWAXAgent\` (plan-v2 §5)

All Agent writes are confined here (asInvoker, no UAC). `Program Files`, `Windows`,
`System32`, and `HKLM` are never touched (plan-v2 §16).

```
%LocalAppData%\HWAXAgent\
 ├─ modules\
 │   └─ <ModuleId>\
 │       ├─ <version>\                  e.g. 1.2.0\  → KooPreprocessor.exe, resources\
 │       │   └─ .install_meta.json      {"sha256":"…","installed_at":"…"}
 │       └─ current.json                ⭐ single truth: which version is active
 │            { "version":"1.2.0", "installed_at":"…Z", "sha256":"…",
 │              "previous_version":"1.1.0" }
 ├─ cache\
 │   ├─ manifest.json                   last synced server manifest snapshot + ETag
 │   └─ downloads\                       *.partial during download
 ├─ config.json                          server, agent_id, auto_update, log_level,
 │                                        allowed_origins, keep_last_n_versions, …
 ├─ logs\
 │   ├─ agent-YYYY-MM-DD.log            daily rolling agent log
 │   ├─ install-<id>-<ver>.log          one file per install attempt
 │   └─ run-<id>-<ts>.log               one file per run
 └─ .lock                                pid + flock — single-instance guard
```

**Secrets are NOT files.** The device JWT and refresh token live in the **Windows
Credential Manager** (`keyring`), never in `config.json` or any plaintext file
(plan-v2 §5, §12). `config.json` records only `agent_id`.

### Manifest ↔ folder mapping
- `manifest.programs[].id` → `modules\<id>\`
- `manifest.programs[].version` → `modules\<id>\<version>\`
- `manifest.programs[].package.sha256` → verified, then copied into both
  `.install_meta.json` and `current.json`.
- Token expansion in `entry.args_template` / `working_dir`: `${MODULE_DIR}` =
  `modules\<id>\<version>`, `${USER_DIR}` = `%LocalAppData%\HWAXAgent`,
  `${AGENT_ID}` = paired id (plan-v2 §7).

---

## 5. Trust boundaries & invariants (plan-v2 §15, §17 — standing acceptance criteria)

1. Downloads only from `config.allowed_origins` (exact origin match) — never a
   user-typed URL. `hwax_core::origin::ensure_allowed`.
2. Every package SHA-256-verified before extract/execute. `hwax_core::hash::verify_file`.
3. Zip extraction is zip-slip-safe. `hwax_core::zip_safe::extract_zip_safe`.
4. `staging → final` and `current.json` writes are atomic (same-volume rename).
   `hwax_core::atomic` + `hwax_core::install::perform_swap`.
5. Execute **only** `manifest.entry.executable` (whitelist), no user-supplied args.
6. Device JWT / refresh token in Credential Manager (keyring), never plaintext.
7. Default `asInvoker`; writes only under `%LocalAppData%\HWAXAgent\`.
8. Tauri allowlist minimal: `shell.open=false`; http scope = single server domain;
   tight CSP (plan-v2 §15.1).
9. Server address is locked in the UI; changing it requires re-pairing
   (plan-v2 §4.4) — this is a core AV-false-positive mitigation.

These nine map 1:1 onto the plan-v2 §17 anti-pattern checklist and are the
code-review gate (`docs/CONTRIBUTING.md`).
