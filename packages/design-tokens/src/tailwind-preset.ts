/**
 * Tailwind preset that exposes the HWAX Agent palette (mirrored from
 * contracts/hwax-agent/tokens.css) as Tailwind theme utilities.
 *
 * Consume it from the Tauri React app's tailwind.config:
 *
 *   import { hwaxPreset } from '@hwax/design-tokens/tailwind-preset';
 *   export default {
 *     presets: [hwaxPreset],
 *     content: ['./index.html', './src/**\/*.{ts,tsx}'],
 *   };
 *
 * Each color is wired to BOTH the baked hex (so the preset works with zero CSS
 * setup) and, where it matters for theming, the design tokens can be swapped to
 * `cssVar(...)` references — see tokens.ts. We default to the hex literals so a
 * consumer that forgets to inject the `:root` block still renders correctly.
 */

import { colors, radius, fonts } from './tokens.js';

/**
 * Minimal structural type for a Tailwind preset so this package builds with
 * plain `tsc` WITHOUT a hard compile-time dependency on `tailwindcss` types.
 * The real `tailwindcss` `Config` is structurally compatible with this subset;
 * the consuming app supplies the full `Config` type at its own call site.
 */
export interface TailwindPreset {
  theme?: {
    extend?: {
      colors?: Record<string, string>;
      borderRadius?: Record<string, string>;
      fontFamily?: Record<string, string[]>;
    };
  };
}

/**
 * Split the comma-separated `fonts.sans` token (e.g.
 * `"'Pretendard Variable', system-ui, sans-serif"`) into the array form
 * Tailwind's `fontFamily` expects.
 */
const sansStack: string[] = fonts.sans.split(',').map((f) => f.trim());

/**
 * The HWAX preset. Colors are namespaced under `hwax-*` so they never collide
 * with a host project's palette:
 *   bg:    bg-hwax-base / bg-hwax-elevated
 *   accent: text-hwax-accent / hover:bg-hwax-accent-hover
 *   text:  text-hwax-primary / text-hwax-muted
 *   border: border-hwax
 * Radius: rounded-hwax-md / rounded-hwax-lg. Font: font-hwax.
 */
export const hwaxPreset: TailwindPreset = {
  theme: {
    extend: {
      colors: {
        'hwax-base': colors.bgBase,
        'hwax-elevated': colors.bgElevated,
        'hwax-accent': colors.accent,
        'hwax-accent-hover': colors.accentHover,
        'hwax-primary': colors.textPrimary,
        'hwax-muted': colors.textMuted,
        // `border-hwax` resolves the single `hwax` color key.
        hwax: colors.border,
      },
      borderRadius: {
        'hwax-md': radius.md,
        'hwax-lg': radius.lg,
      },
      fontFamily: {
        hwax: sansStack,
      },
    },
  },
};

export default hwaxPreset;
