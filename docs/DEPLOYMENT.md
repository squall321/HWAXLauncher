# Deployment & release checklist

End-to-end steps to cut a signed, auto-updatable HWAX Agent release. The
mechanics live in `.github/workflows/build-and-sign.yml`; this is the operator
checklist around it.

## 0. Prerequisites (build machine / CI runner)

- Windows 10 21H2+ / 11, x64.
- Rust ≥ 1.77 (`rust-toolchain.toml` pins the channel + MSVC target).
- Node 20+, pnpm 9 (via `corepack`).
- WebView2 Runtime (preinstalled on Win11; bundled by NSIS for Win10).

> **Low-disk note:** the workspace `target/` is large. If the system drive is
> tight, set `CARGO_TARGET_DIR` to a roomy volume before building.

## 1. Local packaging (dry run)

```powershell
corepack pnpm install
corepack pnpm --filter @hwax/agent build          # → apps/agent/dist
corepack pnpm --filter @hwax/agent tauri build --bundles nsis msi
```

Artifacts land in **`target/release/bundle/`** (workspace-root target — not under
`apps/agent/src-tauri/`, because `src-tauri` is a workspace member):

- `target/release/bundle/nsis/HWAX Agent_<ver>_x64-setup.exe` (per-user)
- `target/release/bundle/msi/HWAX Agent_<ver>_x64_en-US.msi`

## 2. CI secrets & variables (Settings → Secrets and variables → Actions)

Set these once; `build-and-sign.yml` consumes them. **Nothing is hardcoded.**

| name | kind | purpose | how to get it |
|---|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | secret | Ed25519 **updater** signature (plan-v2 §18) | contents of `.tauri/hwax-updater.key` — see [UPDATER-SIGNING.md](UPDATER-SIGNING.md) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | secret | password for the above | empty for the current key |
| `AZURE_KEY_VAULT_URL` | secret | Authenticode cert location | Key Vault URL |
| `AZURE_KEY_VAULT_CERT` | secret | cert name in Key Vault | — |
| `AZURE_CLIENT_ID` / `AZURE_TENANT_ID` | secret | OIDC identity for AzureSignTool | app registration |
| `AZURE_CLIENT_SECRET` | secret | non-OIDC fallback only | optional |
| `HEAXHUB_PUBLISH_TOKEN` | secret | upload to `installer_packages` | HEAXHub admin |
| `HEAXHUB_BASE_URL` | **variable** | server base URL (non-secret) | e.g. `https://hwax.sec.samsung.net/heax-hub` |

Also create the protected **`release`** environment (the signing job gates on it).

## 3. Cut a release

```sh
git tag v0.1.0 && git push origin v0.1.0
```

`build-and-sign.yml` then: gates on `cargo test -p hwax-core` → `tauri build`
(NSIS+MSI, updater-signed) → Authenticode-signs the exe/installers via
AzureSignTool (cert stays in Key Vault, only a short-lived OIDC token crosses
the wire) → SHA-256 → uploads to HEAXHub `installer_packages`. Clients discover
the new version via `GET /api/v1/installers/hwax-agent/latest` and verify the
updater signature against the committed pubkey.

## 4. Per-environment values to finalize before first prod release

- **Server domain (wired to the HWAX Portal):** the launcher connects via the
  **HWAX Portal** at `https://hwax.sec.samsung.net/heax-hub` (the portal strips the
  `/heax-hub` prefix before HEAXHub/Caddy; see HEAXHub `docs/HWAX-PORTAL-INTEGRATION.md`).
  `tauri.conf.json` (updater endpoint + CSP `connect-src`/`img-src`) and the
  pre-pairing fallback now use this host. `config.server` is set at pairing and may
  carry the `/heax-hub` sub-path — the agent appends `/api/v1/...`, so the
  portal-stripped path resolves; the origin allow-list compares scheme+host only,
  so the sub-path is fine.
- **EDR/AV whitelist:** fill the 4 values (install path, `HWAXAgent.exe`, cert
  thumbprint, download domain) — see [EDR-WHITELIST.md](EDR-WHITELIST.md) — and
  submit to the security team **before** rollout, or installs will be quarantined.
- **Channel:** `stable` / `beta` / `dev` (config + updater feed).

## 5. Verify a release

1. Fresh Win VM → run the NSIS `-setup.exe` (no admin prompt — per-user).
2. Tray icon appears (green dot). Pair with an operator-issued enrollment token.
3. Confirm a module installs (download → sha256 → extract → swap) and runs.
4. Confirm `Settings → 진단 dump 만들기` produces a zip under
   `%LocalAppData%\HWAXAgent\diagnostics\`.
5. Bump the version, publish a newer build, confirm the agent self-updates.
