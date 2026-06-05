/**
 * `@hwax/schemas` — TypeScript types + ajv 2020 validators for the HWAX Agent
 * wire contracts. Consumed by the Tauri React app (`@hwax/agent`) and CI.
 *
 * Usage:
 *   import { validateManifest, type Manifest } from '@hwax/schemas';
 *   if (!validateManifest(json)) throw new Error(formatErrors(validateManifest.errors));
 *   const manifest = json as Manifest;
 */

export type {
  // manifest.schema.json
  Manifest,
  Program,
  Package,
  PackageType,
  Entry,
  Requirements,
  Lifecycle,
  PostInstallCheck,
  UiHints,
  Visibility,
  // install-report.schema.json
  InstallReport,
  InstallStatus,
  // audit-event.schema.json
  AuditEvent,
  AuditKind,
  Severity,
  ClientMeta,
} from './types.js';

export {
  validateManifest,
  validateInstallReport,
  validateAuditEvent,
  formatErrors,
} from './validate.js';

export type { ErrorObject, ValidateFunction } from './validate.js';
