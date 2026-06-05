import type { Config } from 'tailwindcss';

/**
 * Theme maps 1:1 onto the design tokens in
 * `contracts/hwax-agent/tokens.css` (dark base + amber accent, Pretendard).
 * Colors are wired through CSS custom properties (defined in styles/index.css)
 * so the token file stays the single source of truth — change a hex there and
 * the whole UI follows.
 */
export default {
  darkMode: 'class',
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        hwax: {
          // --hwax-bg-base
          bg: 'var(--hwax-bg-base)',
          // --hwax-bg-elevated
          elevated: 'var(--hwax-bg-elevated)',
          // --hwax-accent (#f59e0b) + hover (#fbbf24)
          accent: 'var(--hwax-accent)',
          'accent-hover': 'var(--hwax-accent-hover)',
          // --hwax-text-primary / muted
          text: 'var(--hwax-text-primary)',
          muted: 'var(--hwax-text-muted)',
          // --hwax-border
          border: 'var(--hwax-border)',
        },
        // Status dots: green / yellow / red (v2 §4.1).
        status: {
          green: '#22c55e',
          yellow: '#eab308',
          red: '#ef4444',
        },
      },
      borderRadius: {
        // --hwax-radius-md / lg
        md: 'var(--hwax-radius-md)',
        lg: 'var(--hwax-radius-lg)',
      },
      fontFamily: {
        // --hwax-font
        sans: ['Pretendard Variable', 'Pretendard', 'system-ui', 'sans-serif'],
      },
    },
  },
  plugins: [],
} satisfies Config;
