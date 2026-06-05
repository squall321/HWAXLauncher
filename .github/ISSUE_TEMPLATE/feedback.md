---
name: Feedback / request
about: A feature request, UX feedback, or a need for a HEAXHub contract/endpoint change.
title: "[feedback] "
labels: hwax-agent, needs-triage
assignees: ''
---

## What do you want
<!-- e.g. "add a 'rollback_reason' field to the install report",
         "show download speed in the tray", "support proxy auth". -->

## Why — the user scenario
- On a Windows PC, in what situation does this come up?
- Is there a current workaround?

## Does this need a HEAXHub contract / server change?
<!--
If yes, this becomes a cross-repo PR to HEAXHub (CONTRIBUTING §5, scenarios A/B).
Open the tracking issue here, then the contract PR upstream. Check what changes:
-->
- [ ] `manifest.schema.json` (new field the server must send)
- [ ] `install-report.schema.json`
- [ ] `audit-event.schema.json`
- [ ] `openapi.yaml` (new/changed endpoint)
- [ ] `tokens.css` (design token)
- [ ] No contract change — client-only (Tauri/React)

## Estimated SemVer impact on contracts (if any)
- [ ] MAJOR (required field / removed enum / tighter security)
- [ ] MINOR (new optional endpoint / field / enum value)
- [ ] PATCH (docs/examples only)
- [ ] N/A

## Links
- Upstream HEAXHub issue/PR (if filed):
