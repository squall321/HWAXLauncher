# Contributing

The full contributor guide lives at **[`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md)**.

In one breath: the stack is fixed (Tauri 2 + React 18 + TypeScript) and the
forbidden list (WinUI3/WPF/.NET/C#/XAML/MAUI/Avalonia/Electron/Flutter) is a hard
review gate; `contracts/hwax-agent/` is a vendored mirror — never hand-edit it,
re-sync with `pnpm fetch-schemas`; never commit secrets or signing keys; reuse
`hwax-core` for the security-critical logic; and contract/server changes go as PRs
to HEAXHub. See `docs/CONTRIBUTING.md` for the labels, SemVer rules, and the four
cross-repo collaboration scenarios.
