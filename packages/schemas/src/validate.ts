/**
 * Runtime validators for the three HWAX Agent wire contracts, built on ajv's
 * JSON Schema Draft 2020-12 dialect (the dialect the .schema.json files declare).
 *
 * The schemas are loaded from the vendored single source of truth at
 * `contracts/hwax-agent/` — they are NOT duplicated here. This keeps the types
 * (types.ts, hand-written) and the runtime check (these compiled schemas) both
 * pinned to one canonical file, so drift is caught by the vitest suite.
 *
 * `additionalProperties: false` is honoured by ajv, so these validators reject
 * any payload carrying a key the schema forbids — exactly what the agent needs
 * before it serializes a request body or trusts a manifest response.
 */

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

import Ajv2020 from 'ajv/dist/2020.js';
import type { ErrorObject, ValidateFunction } from 'ajv';
import addFormats from 'ajv-formats';

import type { AuditEvent, InstallReport, Manifest } from './types.js';

/**
 * Absolute path to the vendored contracts directory.
 *
 * Resolution is anchored on this module's own location, walking up out of the
 * compiled `dist/` (or `src/` under ts-node/vitest) to the package root, then
 * to the repo's `contracts/hwax-agent/`. Layout:
 *   <repo>/packages/schemas/{src,dist}/validate.(ts|js)  ← here
 *   <repo>/contracts/hwax-agent/*.schema.json            ← target
 */
const HERE = dirname(fileURLToPath(import.meta.url));
const CONTRACTS_DIR = resolve(HERE, '..', '..', '..', 'contracts', 'hwax-agent');

const MANIFEST_SCHEMA = 'manifest.schema.json';
const INSTALL_REPORT_SCHEMA = 'install-report.schema.json';
const AUDIT_EVENT_SCHEMA = 'audit-event.schema.json';

function loadSchema(file: string): Record<string, unknown> {
  const path = resolve(CONTRACTS_DIR, file);
  return JSON.parse(readFileSync(path, 'utf-8')) as Record<string, unknown>;
}

/**
 * A single ajv instance shared by all three validators. Each schema is
 * registered under its `$id`; `format` keywords (date-time, uri, uuid) are
 * checked by ajv-formats.
 */
const ajv = new Ajv2020({
  // Surface every problem, not just the first — useful when reporting to audit.
  allErrors: true,
  // The contract schemas are trusted, vendored inputs; strict mode would reject
  // harmless keywords (e.g. `examples`, `description`) on some drafts. We keep
  // strictness for genuinely meaningful issues but allow annotation keywords.
  strict: false,
});
// ajv-formats is CJS with a default export; the `.default ?? ` guard keeps this
// working under both Node ESM interop and bundlers.
const applyFormats = (addFormats as unknown as { default?: typeof addFormats }).default ?? addFormats;
applyFormats(ajv);

/**
 * Build a typed validator for one schema file. The returned function is an ajv
 * `ValidateFunction<T>`: it returns a boolean and, on failure, exposes
 * `.errors` (an array of {@link ErrorObject}).
 */
function compile<T>(file: string): ValidateFunction<T> {
  return ajv.compile<T>(loadSchema(file));
}

/** Validate a `GET /api/v1/launcher-agents/manifest` response body. */
export const validateManifest: ValidateFunction<Manifest> = compile<Manifest>(MANIFEST_SCHEMA);

/** Validate a `POST /api/v1/launcher-agents/installs` body. */
export const validateInstallReport: ValidateFunction<InstallReport> =
  compile<InstallReport>(INSTALL_REPORT_SCHEMA);

/** Validate a `POST /api/v1/launcher-agents/audit` body. */
export const validateAuditEvent: ValidateFunction<AuditEvent> =
  compile<AuditEvent>(AUDIT_EVENT_SCHEMA);

/**
 * Render ajv errors into a single human-readable line. Handy for logging an
 * invalid payload before dropping it, or for assertion messages in tests.
 * Returns the empty string when there are no errors.
 */
export function formatErrors(errors: ErrorObject[] | null | undefined): string {
  if (!errors || errors.length === 0) return '';
  return errors
    .map((e) => {
      // For `additionalProperties` violations ajv puts the offending key in
      // params.additionalProperty, not in the message — surface it so logs and
      // assertions can see exactly which forbidden key was sent.
      const extra =
        e.keyword === 'additionalProperties' &&
        typeof e.params?.['additionalProperty'] === 'string'
          ? ` '${e.params['additionalProperty'] as string}'`
          : '';
      return `${e.instancePath || '(root)'} ${e.message ?? 'is invalid'}${extra}`;
    })
    .join('; ');
}

export type { ErrorObject, ValidateFunction };
