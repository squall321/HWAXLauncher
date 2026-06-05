# HWAX Agent — Operations Runbook

> For support engineers and ops handling HWAX Agent on user PCs. Pairing,
> troubleshooting, diagnostic dumps, and where the logs live. Architecture
> context: `docs/ARCHITECTURE.md`. EDR allow-listing: `docs/EDR-WHITELIST.md`.

Everything the Agent writes is under **`%LocalAppData%\HWAXAgent\`** (no admin
needed). Secrets are in **Windows Credential Manager**, never a file.

---

## 1. Pairing (first-run enrollment) — plan-v2 §12

Pairing happens once; afterwards every call uses the device JWT.

1. User installs `HWAXAgentSetup` (per-user NSIS/MSI) → it lands in
   `%LocalAppData%`, the tray icon appears, and a balloon says "Pair with HEAXHub".
2. An operator issues a **single-use enrollment token** from the HEAXHub admin UI
   (`POST /api/v1/admin/agents`). The token is shown **once** — record it in the
   internal vault immediately.
3. Deliver the token to the user over an internal channel.
4. In the tray menu → **Pair** → the user pastes the enrollment token. The Agent
   calls `POST /api/v1/launcher-agents/enroll { enrollment_token, hostname,
   agent_version }`.
5. HEAXHub returns `{ agent_id, access_token, refresh_token, expires_in }`. The
   Agent:
   - stores `access_token` + `refresh_token` in **Credential Manager**
     (`HWAXAgent:device_jwt`, `HWAXAgent:refresh_token`),
   - writes only `agent_id` (and `server`) into `config.json`,
   - runs an initial manifest sync → tray dot turns **green**.

**Verify success:** tray tooltip shows "synced N seconds ago"; `config.json` has a
non-empty `agent_id`; Credential Manager has the two `HWAXAgent:*` entries.

### Token lifecycle
- `access_token`: ~3600 s, audience `hwax-agent`.
- `refresh_token`: ~30 days, **rotated on every refresh**.
- On `401`, the Agent calls `POST /api/v1/launcher-agents/refresh { refresh_token }`
  and replaces both tokens in the keyring. Refresh failure ⇒ tray alert + re-pair.

---

## 2. Tray status dots (triage at a glance) — plan-v2 §11

| Dot | Meaning | First action |
|---|---|---|
| 🟢 green | Healthy; last sync recent | none |
| 🟡 yellow | Warning — server unreachable ≥5 consecutive syncs; running on cache | §3.1 |
| 🔴 red | Error — token loss, disk, or repeated install failure | §3.2 / §3.3 |

---

## 3. Troubleshooting

### 3.1 Server unreachable (yellow dot) — plan-v2 §13
**Symptom:** tooltip "synced X min ago" stops advancing; yellow dot after 5 failures.
The Agent keeps working from the last cached manifest — modules already installed
still run.

Triage:
1. From the PC, confirm DNS + reachability of the HEAXHub host (the value of
   `server` in `config.json`). A simple GET to the health endpoint is enough.
2. Check corporate proxy: if traffic must traverse one, `config.proxy` must be set
   (changed only via re-pair / config patch, not free-typed).
3. Confirm `https` and the cert chain — TLS interception appliances can break it.
4. If the **EDR** is silently dropping the connection to the download/API domain,
   see `docs/EDR-WHITELIST.md` item 4 (domain `heaxhub.local` / on-prem host).
5. When connectivity returns, **Sync now** from the tray; the dot returns to green.

### 3.2 AV / EDR blocked a module or the Agent
**Symptom:** download or `post_install_check` fails oddly; the exe vanishes after
download; an `av_blocked_suspect` audit event was emitted (plan-v2 §19).

Triage:
1. Confirm the four allow-list items are registered with the EDR
   (`docs/EDR-WHITELIST.md`): install folder, process name `HWAXAgent.exe`, the
   signing-cert thumbprint, and the download domain.
2. Check the EDR quarantine log for `HWAXAgent.exe` or the module exe by name.
3. Confirm the module's SHA-256 in `current.json` / `install-<id>-<ver>.log`
   matches the manifest — a mismatch is the Agent *correctly* refusing a tampered
   package, not an AV bug.
4. Capture a diagnostic dump (§4) and file the EDR allow-list request per
   `docs/EDR-WHITELIST.md`.

The Agent's own mitigations are already on by design (plan-v2 §15): allow-listed
origin only, SHA-256 enforced, fixed download path, signed binary, locked server
URL. Do **not** work around them (e.g. don't disable SHA-256).

### 3.3 Token loss / re-pairing needed (red dot)
**Symptom:** repeated `401`s; refresh fails; tray prompts to re-pair. Causes:
Credential Manager entries deleted, machine re-imaged, agent disabled server-side.

Triage:
1. Open **Credential Manager → Windows Credentials** and check for
   `HWAXAgent:device_jwt` / `HWAXAgent:refresh_token`. If missing → re-pair (§1).
2. If present but still `401`: the server may have **disabled** this agent row
   (ops disables, never deletes — audit retention). Issue a **new** enrollment
   token and re-pair; do not try to revive the old row.
3. After re-pair, confirm green dot and a fresh sync.

> Re-pairing is also the *only* supported way to change the server address
> (plan-v2 §4.4). There is no free-form server field — by design.

### 3.4 Update failed but previous version preserved — plan-v2 §8/§9
**Symptom:** toast "Update failed. Keeping previous version (X)." This is the
**designed safe path**: staging was discarded and `current.json` was never touched,
so the old version still runs.

Triage:
1. Open `install-<id>-<ver>.log` — it has the `post_install_check` stdout/stderr +
   exit code (or the SHA-256 mismatch / zip-slip reason).
2. If `sha256_mismatch`: the package on the server differs from its recorded hash —
   escalate to HEAXHub (do not retry blindly).
3. If `post_install_check` failed: usually a missing dependency
   (`requirements.depends_on`) or runtime/license issue on the PC.
4. To force a clean retry: **Sync now**, then re-trigger the update. To roll a
   *running* version back manually: module detail → **Rollback** (restores
   `previous_version`; the directory must still be on disk — GC keeps the last N).

### 3.5 Disk / state corruption
- `current.json` invalid → the module shows `not_installed`; reinstall the version.
- `.lock` left over from a crash blocks startup → after confirming no `HWAXAgent.exe`
  is running, the stale lock can be cleared and the Agent restarted.
- Low disk free surfaces in `health_check` and blocks downloads — free space, then
  **Clear cache** (`cache\downloads\*.partial`).

---

## 4. Diagnostic dump — plan-v2 §19

Tray → **Settings → Make diagnostic dump**. Produces
`%temp%\hwax-dump-<timestamp>.zip` containing:
- `agent-*.log` and `install-*.log` (last 7 days),
- `config.json` (anonymized — `agent_id` / `server` kept, no secrets),
- `system.json` (Windows build, disk free),
- `manifest.json` (last cached snapshot).

The dump **never contains the device JWT or refresh token** — those stay in the
keyring and are anonymized out. Explorer opens to the zip so the user can attach it
to a ticket.

---

## 5. Log locations & retention — plan-v2 §19

Folder: **`%LocalAppData%\HWAXAgent\logs\`** (tray → "Open log folder", one click).

| File | Contents |
|---|---|
| `agent-YYYY-MM-DD.log` | general agent log (JSON via `tracing`), daily rolling |
| `install-<id>-<ver>.log` | one file per install attempt — sha256, extract, `post_install_check` |
| `run-<id>-<ts>.log` | one file per module run — captured stdout/stderr |

Retention: 30 days, 1 GB total cap; older logs auto-deleted. Set verbosity in
Settings (`log_level`: trace/debug/info/warn/error).

### Server-side audit (cross-reference)
The Agent also batches audit events to `POST /api/v1/launcher-agents/audit`
(immediate on `installed` / `rolled_back` / `sha256_mismatch` /
`av_blocked_suspect`). When a user reports "it didn't work", correlate the local
`install-*.log` with the server audit log for that `agent_id`.

---

## 6. Quick reference

| Need | Where |
|---|---|
| Logs | `%LocalAppData%\HWAXAgent\logs\` (tray → Open log folder) |
| Config (non-secret) | `%LocalAppData%\HWAXAgent\config.json` |
| Secrets | Windows Credential Manager → `HWAXAgent:device_jwt`, `HWAXAgent:refresh_token` |
| Which version is active | `modules\<id>\current.json` |
| Diagnostic dump | `%temp%\hwax-dump-<ts>.zip` (tray → Settings) |
| Re-pair | Tray → Pair again (only way to change server address) |
| EDR allow-list request | `docs/EDR-WHITELIST.md` |
