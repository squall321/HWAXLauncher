# @hwax/design-tokens

Typed mirror of `contracts/hwax-agent/tokens.css` — the **HWAX Agent** palette
(dark base + amber accent) — plus a **Tailwind preset** that exposes those
tokens as utility classes.

> Single source of truth is **`contracts/hwax-agent/tokens.css`**. The values in
> `src/tokens.ts` are kept byte-identical to its `:root` custom properties. If
> the palette changes, edit `tokens.css` upstream first, then re-mirror here.

> ⚠ The upstream `tokens.css` header comment mentions a "WinUI3 launcher" and an
> "XAML resource dictionary". That comment is **stale** (see
> `contracts/hwax-agent/SYNC.md`). HWAXLauncher is **Tauri 2 + React + Tailwind**
> — we mirror the same color *values* into TypeScript + a Tailwind preset, not
> into XAML. Ignore the C#/WinUI3 wording.

## Install / build

This is a workspace package (`packages/design-tokens`). It builds with plain
`tsc`:

```sh
pnpm install
pnpm --filter @hwax/design-tokens build
```

Output is ESM + `.d.ts` in `dist/`.

## Tokens

| Token | Value | CSS variable |
|---|---|---|
| `colors.bgBase` | `#0a0a0a` | `--hwax-bg-base` |
| `colors.bgElevated` | `#161618` | `--hwax-bg-elevated` |
| `colors.accent` | `#f59e0b` | `--hwax-accent` |
| `colors.accentHover` | `#fbbf24` | `--hwax-accent-hover` |
| `colors.textPrimary` | `#fafafa` | `--hwax-text-primary` |
| `colors.textMuted` | `#a3a3a3` | `--hwax-text-muted` |
| `colors.border` | `#27272a` | `--hwax-border` |
| `radius.md` | `8px` | `--hwax-radius-md` |
| `radius.lg` | `12px` | `--hwax-radius-lg` |
| `fonts.sans` | `'Pretendard Variable', system-ui, sans-serif` | `--hwax-font` |

## Usage

### As typed values

```ts
import { colors, radius, cssVar, toCssVariables } from '@hwax/design-tokens';

colors.accent;            // "#f59e0b"  (baked hex)
cssVar('accent');         // "var(--hwax-accent)"  (runtime, themeable)
toCssVariables();         // ":root { --hwax-bg-base: #0a0a0a; ... }"
```

`toCssVariables()` regenerates the exact `:root` block from this typed source, so
the app can inject it once and keep the runtime CSS variables in lock-step with
`tokens.css`.

### As a Tailwind preset

```ts
// apps/agent/tailwind.config.ts
import type { Config } from 'tailwindcss';
import { hwaxPreset } from '@hwax/design-tokens/tailwind-preset';

export default {
  presets: [hwaxPreset],
  content: ['./index.html', './src/**/*.{ts,tsx}'],
} satisfies Config;
```

Generated utility classes (namespaced `hwax-*` to avoid collisions):

| Utility | Maps to |
|---|---|
| `bg-hwax-base` / `bg-hwax-elevated` | background colors |
| `text-hwax-accent` / `hover:bg-hwax-accent-hover` | amber accent |
| `text-hwax-primary` / `text-hwax-muted` | text colors |
| `border-hwax` | `#27272a` border |
| `rounded-hwax-md` / `rounded-hwax-lg` | `8px` / `12px` |
| `font-hwax` | Pretendard stack |
```
