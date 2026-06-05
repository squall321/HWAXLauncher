---
name: Bug report
about: Something in HWAX Agent is broken (install, sync, run, tray, pairing).
title: "[bug] "
labels: hwax-agent, bug, needs-triage
assignees: squall321
---

## What happened
<!-- One or two sentences. What did the Agent do vs. what you expected. -->

## Tray status dot at the time
- [ ] 🟢 green   [ ] 🟡 yellow (unreachable)   [ ] 🔴 red (error)

## Steps to reproduce
1.
2.
3.

## Environment
- Windows version (e.g. Win11 23H2 x64):
- HWAX Agent version (tray → "HWAX Agent · X.Y.Z"):
- Module id + version (if module-specific):
- HEAXHub server (`config.json` → `server`):

## Logs / diagnostics
<!--
Do NOT paste secrets. The device JWT lives in Credential Manager, not in logs.
Attach a diagnostic dump (tray → Settings → Make diagnostic dump → it is anonymized),
and the relevant log file from %LocalAppData%\HWAXAgent\logs\:
  - agent-YYYY-MM-DD.log
  - install-<id>-<ver>.log   (for install/update failures)
  - run-<id>-<ts>.log        (for run failures)
See docs/RUNBOOK.md.
-->

## Suspected category (optional)
- [ ] Pairing / token (RUNBOOK §1, §3.3)
- [ ] Server unreachable (RUNBOOK §3.1)
- [ ] AV / EDR block (RUNBOOK §3.2, EDR-WHITELIST.md)
- [ ] Update failed / rollback (RUNBOOK §3.4)
- [ ] sha256 mismatch (package integrity — escalate to HEAXHub)
- [ ] Other
