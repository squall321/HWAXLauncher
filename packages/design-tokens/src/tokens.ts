/**
 * Typed mirror of `contracts/hwax-agent/tokens.css` — the HWAX Agent palette
 * (dark base + amber accent). tokens.css is the SINGLE SOURCE OF TRUTH; the
 * values below are kept byte-identical to its `:root` custom properties.
 *
 * NOTE on the upstream comment in tokens.css mentioning a "WinUI3 launcher" /
 * "XAML resource dictionary": that comment is STALE (see
 * contracts/hwax-agent/SYNC.md). HWAXLauncher is Tauri 2 + React + Tailwind;
 * we mirror the same color VALUES, just into TypeScript + a Tailwind preset
 * instead of XAML. The palette itself is authoritative and unchanged.
 *
 * Why mirror in TS rather than import the .css? The Tailwind preset and any
 * design-system code need the raw hex/length values at config time (before the
 * browser parses CSS variables), and the Rust/Tauri side and Storybook can
 * consume a plain object too. The CSS variables remain the runtime carrier; this
 * module is the build-time twin.
 */

/** The exact CSS custom-property names declared in tokens.css `:root`. */
export const CSS_VAR_NAMES = {
  bgBase: '--hwax-bg-base',
  bgElevated: '--hwax-bg-elevated',
  accent: '--hwax-accent',
  accentHover: '--hwax-accent-hover',
  textPrimary: '--hwax-text-primary',
  textMuted: '--hwax-text-muted',
  border: '--hwax-border',
  radiusMd: '--hwax-radius-md',
  radiusLg: '--hwax-radius-lg',
  font: '--hwax-font',
} as const;

export type CssVarName = (typeof CSS_VAR_NAMES)[keyof typeof CSS_VAR_NAMES];

/** Color tokens (hex), mirrored from tokens.css. */
export const colors = {
  /** `--hwax-bg-base` */
  bgBase: '#0a0a0a',
  /** `--hwax-bg-elevated` */
  bgElevated: '#161618',
  /** `--hwax-accent` */
  accent: '#f59e0b',
  /** `--hwax-accent-hover` */
  accentHover: '#fbbf24',
  /** `--hwax-text-primary` */
  textPrimary: '#fafafa',
  /** `--hwax-text-muted` */
  textMuted: '#a3a3a3',
  /** `--hwax-border` */
  border: '#27272a',
} as const;

/** Corner-radius tokens, mirrored from tokens.css. */
export const radius = {
  /** `--hwax-radius-md` */
  md: '8px',
  /** `--hwax-radius-lg` */
  lg: '12px',
} as const;

/** Typography tokens, mirrored from tokens.css. */
export const fonts = {
  /** `--hwax-font` */
  sans: "'Pretendard Variable', system-ui, sans-serif",
} as const;

/**
 * All tokens grouped under one object — convenient for design-system consumers
 * and snapshot tests.
 */
export const tokens = {
  colors,
  radius,
  fonts,
} as const;

export type ColorToken = keyof typeof colors;
export type RadiusToken = keyof typeof radius;
export type FontToken = keyof typeof fonts;
export type Tokens = typeof tokens;

/**
 * Re-emit the tokens as a `:root { --hwax-*: value }` CSS block. Lets the React
 * app generate the exact same CSS that lives in contracts/hwax-agent/tokens.css
 * from this typed source, so the two never silently diverge.
 */
export function toCssVariables(): string {
  const lines: string[] = [
    `  ${CSS_VAR_NAMES.bgBase}: ${colors.bgBase};`,
    `  ${CSS_VAR_NAMES.bgElevated}: ${colors.bgElevated};`,
    `  ${CSS_VAR_NAMES.accent}: ${colors.accent};`,
    `  ${CSS_VAR_NAMES.accentHover}: ${colors.accentHover};`,
    `  ${CSS_VAR_NAMES.textPrimary}: ${colors.textPrimary};`,
    `  ${CSS_VAR_NAMES.textMuted}: ${colors.textMuted};`,
    `  ${CSS_VAR_NAMES.border}: ${colors.border};`,
    `  ${CSS_VAR_NAMES.radiusMd}: ${radius.md};`,
    `  ${CSS_VAR_NAMES.radiusLg}: ${radius.lg};`,
    `  ${CSS_VAR_NAMES.font}: ${fonts.sans};`,
  ];
  return `:root {\n${lines.join('\n')}\n}\n`;
}

/**
 * `var(--hwax-…)` reference for a given token, so app code can opt into the
 * runtime CSS variable (themeable) instead of the baked hex literal.
 *
 * @example cssVar('accent') // "var(--hwax-accent)"
 */
export function cssVar(name: keyof typeof CSS_VAR_NAMES): string {
  return `var(${CSS_VAR_NAMES[name]})`;
}
