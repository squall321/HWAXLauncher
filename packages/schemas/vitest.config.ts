import { defineConfig } from 'vitest/config';

/**
 * Vitest runs the `*.test.ts` files (which tsc excludes from the dist build).
 * The validators read the vendored schemas from contracts/hwax-agent/ via fs at
 * runtime, so tests must run from this package dir (the default) for the
 * relative path anchoring in validate.ts to resolve.
 */
export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
