# ADR 0001 — Tauri 2 over WinUI 3 / WPF / .NET for HWAX Agent

- Status: **Accepted**
- Date: 2026-06-05
- Deciders: HEAXHub Platform Team (@squall321)
- Supersedes: the 1st-pass "panel catalog" launcher concept (`hwax-launcher-plan.md`)
- Authoritative source: HEAXHub `docs/hwax-launcher-plan-v2.md` (the implementer's
  constitution) §1, §15, §16, §17, §21
- Rejected-alternative record: HEAXHub `docs/hwax-launcher-plan-winui3.md`
  (kept only for decision history)

> **This ADR is the canonical record of *why* the stack is what it is.** It is
> deliberately one-way: WinUI3 / WPF / .NET / C# / XAML / MAUI / Avalonia /
> Electron / Flutter are **out of scope and forbidden**. If you find a stale
> reference to C#/WinUI3 anywhere in the vendored contracts (e.g. an incidental
> "regenerate the C# DTOs" comment in `contracts/hwax-agent/openapi.yaml`, or a
> `tokens.css` build note), it is **upstream noise** — see
> `contracts/hwax-agent/SYNC.md`. Do not act on it.

---

## Context

HWAX Agent is the **Windows client of HEAXHub**: a tray-resident agent that pulls
a signed program manifest, downloads internal CAE / preprocessor / plugin modules
from an allow-listed origin, verifies them by SHA-256, swaps versions atomically,
and launches a whitelisted executable. The essence is **"a background agent that
quietly and reliably fetches modules"**, *not* "a pretty Windows-only app".

The user fixed six priorities (plan-v2 §1), ranked **above** "ease of security-team
approval":

1. Speed to build
2. Ease of update / distribution
3. Doesn't break on the user's PC
4. Logging / recovery
5. No antivirus / EDR false positives
6. Future extensibility (incl. mac / Linux)

Three candidate stacks were scored against those priorities.

## Decision

**Adopt Tauri 2 (Rust core) + React 18 + TypeScript + Vite + Tailwind.**

The correctness-critical logic lives in a pure, Tauri-independent crate
(`crates/hwax-core`) so it can be tested headlessly; the Tauri shell
(`apps/agent/src-tauri`) is a thin adapter that supplies IO (HTTP, keyring, tray,
process spawn). See ADR 0002-candidate / `docs/ARCHITECTURE.md` for the layering.

## Options considered (plan-v2 §1 scorecard)

| Priority | Tauri 2 | WPF | WinUI 3 |
|---|---|---|---|
| ① Build speed | ★★★★★ (reuse React/TS skills) | ★★★ (XAML learning curve) | ★★ (unstable + learning) |
| ② Update / distribution | ★★★★★ (built-in updater + Ed25519) | ★★ (Squirrel/MSI by hand) | ★★ (MSIX policy friction) |
| ③ Robustness | ★★★★ (Rust memory safety) | ★★★★ (.NET maturity) | ★★ (runtime issues) |
| ④ Logging / recovery | ★★★★ (`tracing`, JSON) | ★★★★ (Serilog) | ★★★ |
| ⑤ AV false positives | ★★★★ (single signed exe) | ★★★★ (single signed exe) | ★★ (MSIX package reputation) |
| ⑥ Extensibility (mac/Linux) | ★★★★★ | ★ | ★ |
| **Overall** | **85%** | 65% | 50% |

### Why Tauri 2 won
- **①+⑥** — the HEAXHub web front end is already React 18 + TypeScript. Tauri lets
  us reuse that UI surface (~95% of the surface is React/TS; ~5% is Rust). The
  same UI later ports to mac/Linux with only the installer adapter swapped
  (plan-v2 §25).
- **②** — `tauri-plugin-updater` with Ed25519 signature verification gives
  staged auto-update + rollback essentially for free (plan-v2 §18).
- **③** — the Rust core gives memory safety on exactly the dangerous paths
  (download, unzip, swap).
- **⑤** — ships as a **single signed exe** (NSIS/MSI per-user), which is the
  friendliest shape for internal EDR allow-listing (plan-v2 §15, `docs/EDR-WHITELIST.md`).

### Why WPF was rejected
Strong runtime maturity (③/④) and a single signed exe (⑤), but it fails ⑥
(no mac/Linux path) and ① (cannot reuse the React/TS investment — XAML rewrite).

### Why WinUI 3 was rejected
Field reports on ②/③ are inconsistent; MSIX packaging brings policy friction and
**reputation-based AV heuristics** (⑤) — the single most important priority for an
internal deployment agent. It also fails ⑥.

### Why Electron was not even scored
Bundle size and per-app Chromium reputation make ⑤ worse than WinUI3, and it
gives nothing Tauri does not. Explicitly forbidden alongside the .NET family.

## Consequences

- **Positive:** small signed binary, headless-testable core, one UI codebase for
  future OSes, built-in signed updater.
- **Cost:** the team carries a thin slice of Rust. Mitigated because the Rust
  surface is confined to `hwax-core` patterns (plan-v2 §8) plus shell adapters.
- **Constraint (binding on all future PRs):** the forbidden-stack list above is a
  review gate. A PR introducing WinUI3 / WPF / .NET / C# / XAML / MAUI / Avalonia
  / Electron / Flutter is rejected on sight (`docs/CONTRIBUTING.md`).
- **Security posture inherited:** all of plan-v2 §15 (AV/EDR avoidance) and §17
  (anti-pattern checklist) become standing acceptance criteria.

## Links

- HEAXHub `docs/hwax-launcher-plan-v2.md` — §1 scorecard, §15/§16/§17, §18, §21, §25
- HEAXHub `docs/hwax-launcher-plan-winui3.md` — the rejected WinUI3 design (history)
- `docs/ARCHITECTURE.md` — the hwax-core ↔ src-tauri layering this decision implies
- `contracts/hwax-agent/SYNC.md` — why stale C#/WinUI3 comments must be ignored
