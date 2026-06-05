# HWAX Agent — EDR / Antivirus Allow-List Guide

> For the security / endpoint-protection team. Source: HEAXHub
> `docs/hwax-launcher-plan-v2.md` §15 (AV/EDR false-positive avoidance) and §15.2.
>
> **Why this matters (plan-v2 §15):** if the internal EDR (Kaspersky, Microsoft
> Defender for Endpoint, SentinelOne, …) blocks the Agent or a module it deploys,
> the Agent's value drops to zero. Pre-registering **four** items dramatically
> lowers the chance of a heuristic false positive.

The Agent is built to be allow-list-friendly: it runs as a **single signed exe**
under the user profile (no admin), downloads only from one allow-listed origin,
enforces SHA-256 on every package, fixes the download path, and **locks the server
URL** (no free-form URL input). See "Posture" below.

---

## The four items to allow-list (plan-v2 §15.2)

| # | Item | Value (fill in per environment) |
|---|---|---|
| 1 | **Install folder** | per-user install path, e.g. `%LocalAppData%\Programs\HWAXAgent\` (and runtime data `%LocalAppData%\HWAXAgent\`) |
| 2 | **Process name** | `HWAXAgent.exe` |
| 3 | **Signing certificate thumbprint** | `<SHA-1 / SHA-256 thumbprint of the code-signing cert>` — see "Signing posture" |
| 4 | **Download domain** | the HEAXHub on-prem host, e.g. `heaxhub.local` (must match `config.allowed_origins`) |

> Items 1–2 scope the Agent process; item 3 lets the EDR trust *anything we sign*
> (Agent + module exes) by publisher; item 4 stops the EDR from silently dropping
> the HTTPS connection to the API / installer endpoints.

### 1. Install folder
Per-user install (NSIS/MSI), no `Program Files`, no admin (plan-v2 §16). Two paths
to allow:
- **Program files:** `%LocalAppData%\Programs\HWAXAgent\` (the Tauri bundler
  per-user default — confirm against the actual installer).
- **Runtime data:** `%LocalAppData%\HWAXAgent\` (modules, cache, logs, config).
  Module exes are launched from `%LocalAppData%\HWAXAgent\modules\<id>\<ver>\`, so
  if the EDR allow-lists by execution path, include this subtree.

### 2. Process name
The tray process is `HWAXAgent.exe`. It is the only Agent process (single instance,
guarded by `.lock`). In Phase 1–2 there is **no service**; the Phase 3+ optional
`HWAXAgent Service` (LocalSystem) is out of scope here (plan-v2 §16.1).

### 3. Signing certificate thumbprint
Both the Agent binary **and** the exes inside each module zip are signed with the
internal PKI / EV code-signing certificate (plan-v2 §15 item ④). Allow-listing by
**publisher / thumbprint** is the most durable rule — it survives version bumps,
because the thumbprint is stable while file hashes change every release.

To obtain the thumbprint of a released, signed binary:

```powershell
# Thumbprint (SHA-1) of the signing cert on a signed file:
(Get-AuthenticodeSignature 'HWAXAgent.exe').SignerCertificate.Thumbprint

# Full signature detail (publisher, timestamp, chain status):
Get-AuthenticodeSignature 'HWAXAgent.exe' | Format-List *
```

### 4. Download domain
The Agent downloads installers **only** from origins listed in
`config.allowed_origins`, which is set at pairing time to the HEAXHub server origin
(e.g. `https://heaxhub.local`). A user **cannot** type an arbitrary URL
(plan-v2 §4.4 / §15 item ⑧). Allow-list this single domain for both the API
endpoints (`/api/v1/launcher-agents/*`, `/api/v1/installers/{id}/download`) and the
presigned object-storage host the download 302-redirects to.

---

## How to request the allow-list entry

1. Collect the four values above for the target environment (the install path and
   domain are deployment-specific; the thumbprint comes from the signed release).
2. Attach a **diagnostic dump** if a false positive already occurred
   (`docs/RUNBOOK.md` §4) plus the EDR's quarantine entry for `HWAXAgent.exe` or
   the module exe.
3. File the request with the endpoint-protection team referencing this document,
   stating the scope: *publisher allow-list by thumbprint (item 3) is preferred*;
   path/process rules (items 1–2) and the domain rule (item 4) are the fallback.
4. After registration, re-run a clean install + `post_install_check` to confirm no
   quarantine, and note the resolved request ID for audit.

---

## Signing posture (plan-v2 §15, §21; split-strategy §8)

- **What is signed:** the Agent installer/exe and the exes shipped inside module
  zips. Signing happens **only in the release build** in CI; dev builds may use a
  self-signed test cert (split-strategy §8.2).
- **Where the key lives:** Azure Key Vault (or an internal HSM), fetched at build
  time via a short-lived OIDC token. The signing key/cert **never** enters the repo
  — `.gitignore` blocks `*.key/*.pfx/*.p12/*.pem` and the signing step references
  CI **secrets** only (`.github/workflows/build-and-sign.yml`, `scripts/sign.ps1`).
- **HEAXHub holds no signing key** — it only records each package's `sha256` and a
  `signed` flag (split-strategy §8.1). The signature is baked into the binary; the
  server just hands out a presigned URL.

## SHA-256 posture (defense in depth)

Allow-listing reduces false positives; **SHA-256 is the integrity guarantee** and
is independent of it. The Agent computes the package SHA-256 and compares it to
`manifest.programs[].package.sha256` **before** extracting or executing anything
(`hwax_core::hash::verify_file`). A mismatch is rejected and audited as
`sha256_mismatch` (plan-v2 §8). **Never** ask the team to relax SHA-256 to "make a
block go away" — a mismatch means the package is wrong, not that the check is wrong.

---

## Summary card (hand this to the EDR team)

```
Product           : HWAX Agent (HEAXHub Windows client)
Process           : HWAXAgent.exe   (single per-user process, asInvoker, no admin)
Install path      : %LocalAppData%\Programs\HWAXAgent\
Runtime data path : %LocalAppData%\HWAXAgent\  (modules\, cache\, logs\)
Publisher / cert  : <internal PKI / EV thumbprint>     ← preferred allow-list rule
Network domain    : https://heaxhub.local  (API + presigned installer downloads)
Integrity         : every package SHA-256-verified before extract/execute
Privilege         : standard user; writes confined to %LocalAppData%
```
