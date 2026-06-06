# Updater signing key

The agent self-update flow (`tauri-plugin-updater`, v2 plan §18) verifies every
update package against an **ed25519 / minisign** signature. The keypair was
generated with `tauri signer generate`.

## What is committed vs not

| artifact | location | committed? |
|---|---|---|
| **Public key** | `apps/agent/src-tauri/tauri.conf.json` → `plugins.updater.pubkey` | ✅ yes (it is public) |
| **Private key** | `.tauri/hwax-updater.key` | ❌ never — `.tauri/` is git-ignored |
| Private key password | (this key has an empty password) | ❌ |

> ⚠ The private key in `.tauri/` exists only on the machine that generated it.
> **Move it into a CI secret and do not rely on the local copy.** If you lose
> the private key you cannot sign updates and clients on the old pubkey will
> stop updating until you ship a build with a new pubkey.

## Releasing (CI)

`.github/workflows/build-and-sign.yml` signs the bundle when these secrets are
set on the repo (Settings → Secrets and variables → Actions):

- `TAURI_SIGNING_PRIVATE_KEY` — the **contents** of `.tauri/hwax-updater.key`
  (base64 string), not a path.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — empty for this key (set only if you
  regenerate with a password).

`pnpm tauri build` then emits `*.sig` files next to the installers, and the
updater manifest served at `GET /api/v1/installers/hwax-agent/latest`
(HEAXHub-hosted) carries the signature the client checks against the committed
pubkey.

## Rotating the key

```sh
pnpm tauri signer generate -w .tauri/hwax-updater.key --force
# copy the printed public key into tauri.conf.json plugins.updater.pubkey
# update the TAURI_SIGNING_PRIVATE_KEY CI secret
```

Rotation is a breaking change for clients still on the old pubkey: ship the new
pubkey in a build signed with the **old** key first, let clients update, then
switch signing to the new key.
