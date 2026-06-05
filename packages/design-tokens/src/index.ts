/**
 * `@hwax/design-tokens` — the HWAX Agent dark + amber palette mirrored from
 * contracts/hwax-agent/tokens.css, plus a Tailwind preset. Consumed by the
 * Tauri React app (`@hwax/agent`) and CI.
 */

export {
  CSS_VAR_NAMES,
  colors,
  radius,
  fonts,
  tokens,
  toCssVariables,
  cssVar,
} from './tokens.js';

export type {
  CssVarName,
  ColorToken,
  RadiusToken,
  FontToken,
  Tokens,
} from './tokens.js';

export { hwaxPreset } from './tailwind-preset.js';
export type { TailwindPreset } from './tailwind-preset.js';
