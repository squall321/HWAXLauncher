<!--
HWAXLauncher (HWAX Agent) PR. Mirrors HEAXHub's template, adapted to the Windows
client. See docs/CONTRIBUTING.md. The stack is FIXED — no WinUI3/WPF/.NET/C#/XAML/
MAUI/Avalonia/Electron/Flutter.
-->

## Summary
- One-line change summary:
- Scope (check all that apply):
  - [ ] `apps/agent/src` (React/TS UI)
  - [ ] `apps/agent/src-tauri` (Tauri shell)
  - [ ] `crates/hwax-core` (pure logic)
  - [ ] `packages/` (design-tokens / schemas)
  - [ ] `docs/`
  - [ ] `.github/` / `scripts/` (CI / build)
  - [ ] `contracts/hwax-agent` (vendored — see "Contracts" below; usually a RE-SYNC, not a hand-edit)

## Related issues / links
- HWAXLauncher issue:
- Upstream HEAXHub PR (if this needs a contract/server change):

## Contracts (only if `contracts/hwax-agent/**` changed)
- [ ] This is a **re-sync** via `pnpm fetch-schemas` to a pinned tag — NOT a hand-edit
- [ ] The matching upstream HEAXHub contract PR is merged and linked above
- [ ] Contract `VERSION` / `CHANGELOG.md` bumped per SemVer (CONTRIBUTING §7)
- [ ] `cargo test -p hwax-core` passes (schema conformance gate)

## Verification
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy -p hwax-core --all-targets -- -D warnings` clean
- [ ] `cargo test -p hwax-core` passes
- [ ] `pnpm -r typecheck` / `pnpm -r build` pass
- [ ] Smoke-tested on Win10 and/or Win11 (note which):

## Security checklist (plan-v2 §15 / §17 — a violation blocks merge)
- [ ] No forbidden stack introduced (WinUI3/WPF/.NET/C#/XAML/MAUI/Avalonia/Electron/Flutter)
- [ ] No secret/token/key plaintext in the diff; no signing artifact staged (`*.key/*.pfx/*.p12/*.pem`)
- [ ] Downloads only from `config.allowed_origins` (no user-typed URL)
- [ ] Packages SHA-256-verified before extract/execute
- [ ] Zip extraction stays zip-slip-safe (reuses `hwax_core::zip_safe`)
- [ ] `staging → final` and `current.json` writes stay atomic (reuses `hwax_core`)
- [ ] Executes only `manifest.entry.executable`; no arbitrary exe; no user-supplied args
- [ ] Device JWT / refresh token via Credential Manager (keyring), never a file
- [ ] Default `asInvoker`; writes confined to `%LocalAppData%\HWAXAgent\`
- [ ] Pure logic reuses `hwax-core` (NOT reimplemented)

## Labels
<!-- hwax-agent · contracts · schema-change · breaking · security · ux · perf · needs-heaxhub-change -->
