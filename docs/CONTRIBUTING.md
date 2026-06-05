# Contributing to HWAXLauncher (HWAX Agent)

> This mirrors the cross-repo PR protocol in HEAXHub
> `docs/hwax-agent-pr-protocol.md` and the split strategy in
> `docs/hwax-agent-split-strategy.md`, adapted to **this** (the Windows client)
> repo. The HEAXHub repo is the server and the owner of the contract surface.

A symlinked copy lives at the repo root (`CONTRIBUTING.md`) so GitHub surfaces it
on PRs; this `docs/` copy is the canonical text.

---

## 1. The forbidden-stack rule (non-negotiable)

The stack is **fixed**: Tauri 2 (Rust core) + React 18 + TypeScript + Vite +
Tailwind. The following are **rejected and forbidden** — a PR introducing any of
them is closed on sight (`docs/adr/0001-tauri-2-over-winui3.md`):

> **WinUI 3 · WPF · .NET · C# · XAML · MAUI · Avalonia · Electron · Flutter.**

If you see a C#/WinUI3 reference in the vendored contracts (e.g. an incidental
"regenerate the C# DTOs" comment in `contracts/hwax-agent/openapi.yaml`, or a build
note in `tokens.css`), it is **stale upstream noise** — ignore it and do **not**
"fix" it here. Fix it upstream instead. See `contracts/hwax-agent/SYNC.md`.

## 2. Contracts are the single source of truth

`contracts/hwax-agent/` is a **vendored, read-only mirror** of the source of truth
that lives in the HEAXHub repo. It is pinned to a release tag and synced with
`pnpm fetch-schemas` (`scripts/fetch-schemas.mjs`); in the real workflow it is a
git submodule.

- **Do not hand-edit** files under `contracts/hwax-agent/`. To change the wire
  format you change it **upstream in HEAXHub** (see §5), then re-sync here.
- Only the **wire format** is authoritative: `manifest.schema.json`,
  `install-report.schema.json`, `audit-event.schema.json`, `openapi.yaml`,
  `tokens.css`. Every object is `additionalProperties: false` — the Agent must
  never send a key not in the schema.
- The repo's contract gate is `cargo test -p hwax-core`, which includes
  `tests/schema_conformance.rs` validating built payloads against the real schemas
  (`.github/workflows/contracts-validate.yml`).

### Endpoint prefix
All launcher HTTP calls go under **`/api/v1/launcher-agents/*`** — never the bare
`/api/v1/agents/*` (that is the pre-existing service-agent API). The only shared
path is `GET /api/v1/installers/{id}/download`.

## 3. Never commit secrets or signing keys

- The device JWT / refresh token live in **Windows Credential Manager** at runtime,
  never in a file (plan-v2 §5, §12).
- Code-signing keys/certs are fetched in CI from Key Vault via short-lived OIDC;
  they **never** enter the repo. `.gitignore` blocks `*.key/*.pfx/*.p12/*.pem/*.snk`
  and `secrets/`. The signing step (`scripts/sign.ps1`,
  `.github/workflows/build-and-sign.yml`) references CI **secrets** only.
- Every PR self-checks: "no secret/token plaintext in the diff; no signing artifact
  staged" (pull request template).

## 4. Security review gate — plan-v2 §17 anti-patterns

These eight are a hard review checklist; a PR violating any one is rejected
(plan-v2 §17, mirrored in `docs/ARCHITECTURE.md` §5):

| Anti-pattern | Required behavior |
|---|---|
| Auto-install into `C:\Program Files` | `%LocalAppData%` only |
| Auto-requesting admin | `asInvoker`; `runas` only on explicit user click |
| Editing the registry | only `HKCU\…\Run` by user consent; never `HKLM` |
| Auto-registering a service | not in Phase 1–2 |
| Running an arbitrary exe | only `manifest.entry.executable` |
| Downloading a user-typed URL | reject anything outside `allowed_origins` |
| Running an unverified exe | SHA-256 (+ signature) verified first |
| Blind auto-update overwrite | staging + `post_install_check` + atomic swap |

Reuse `hwax-core` for these (`origin::ensure_allowed`, `hash::verify_file`,
`zip_safe::extract_zip_safe`, `atomic::*`, `install::perform_swap/rollback`); do
**not** reimplement the pure logic.

## 5. Cross-repo PRs to HEAXHub (the 4 collaboration scenarios)

When a change requires a contract or server change, it is **a PR to HEAXHub**, not
a local edit (PR-protocol §2). Open a tracking issue here first (label
`needs-heaxhub-change`), then a PR upstream linking it. The four scenarios:

| Scenario | Trigger | Filed by | Outcome |
|---|---|---|---|
| **A** | HWAXAgent needs a new endpoint | HWAXAgent maintainer | PR to HEAXHub: `openapi.yaml` + stub handler; labels `hwax-agent`, `contracts`, `enhancement` |
| **B** | HWAXAgent needs a new manifest field | HWAXAgent maintainer | PR to HEAXHub: `manifest.schema.json` (+ next-version) additive/optional; labels `hwax-agent`, `contracts`, `schema-change` |
| **C** | HEAXHub changes a server model/enum | HEAXHub maintainer | HEAXHub PR + CHANGELOG; dispatch auto-creates an issue here (`incoming-contract-change`, `from-heaxhub`) |
| **D** | HEAXHub changes a security policy | HEAXHub security owner | `SECURITY.md` + CHANGELOG (`security`); manual follow-up issue here, 2 reviewers |

Rules of thumb (PR-protocol §3): new endpoint calls hide behind a **feature flag**
until the server ships; new manifest fields are parsed as **optional**; the Agent
must **gracefully degrade** on an older contract and never crash on an unknown enum.

### Sync direction & version compatibility (split-strategy §10)
When the Agent requires contracts `vA`, HEAXHub must already deploy **≥ vA**
(the server is always more lenient — forward compatibility). After an upstream
contract merge, re-sync here with `pnpm fetch-schemas` to a pinned tag.

## 6. Labels (kept identical across both repos — PR-protocol §4)

| Label | Use |
|---|---|
| `hwax-agent` | HWAXAgent-related change |
| `contracts` | touches `contracts/` (vendored here) |
| `schema-change` | manifest/install-report/audit-event schema change |
| `breaking` | forces a SemVer major |
| `security` | auth / crypto / policy |
| `ux` | launcher UX |
| `perf` | latency / memory / disk IO |
| `incoming-contract-change` | auto-created from a HEAXHub dispatch |
| `from-heaxhub` / `from-hwax-agent` | originating repo |
| `needs-heaxhub-change` | this repo needs an upstream contract/server change |

## 7. SemVer (contracts) — PR-protocol §6

| Change | contracts bump | HWAXAgent forced bump |
|---|---|---|
| New optional endpoint | minor | minor |
| New optional field on an existing endpoint | patch | patch |
| Add/remove a **required** field | major | major |
| Add an enum value | minor | minor |
| Remove / re-mean an enum value | major | major |
| Tighten a security policy (shorter expiry, algo swap) | major | major |
| Docs/comments only | patch / none | none |

A contracts **major** bump requires **both** maintainers (HEAXHub lead +
HWAXAgent lead) to approve, enforced by CODEOWNERS on `contracts/`.

## 8. Review ownership — `.github/CODEOWNERS`

Default owner is `@squall321`. Anything under `contracts/` additionally requires
review because it is the shared boundary (even though it is vendored — a local
diff there is almost always a mistake and should be a re-sync instead).

## 9. Local checklist before opening a PR

- [ ] Stack rule respected (no forbidden tech; §1).
- [ ] No hand-edits under `contracts/` (re-sync instead; §2).
- [ ] No secret/token plaintext; no signing artifact staged (§3).
- [ ] None of the eight anti-patterns introduced (§4); reuse `hwax-core`.
- [ ] `cargo fmt --check` and `cargo clippy -- -D warnings` clean.
- [ ] `cargo test -p hwax-core` passes (includes schema conformance).
- [ ] `pnpm -r typecheck` / `pnpm -r build` pass.
- [ ] If a contract/server change is needed, an upstream HEAXHub PR is linked (§5).
- [ ] CHANGELOG / user-facing note updated where relevant.
