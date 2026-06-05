/**
 * Hand-written TypeScript mirror of the HWAX Agent wire contracts.
 *
 * SOURCE OF TRUTH — these interfaces mirror, field-for-field:
 *   - contracts/hwax-agent/manifest.schema.json       (HWAXAgentManifest)
 *   - contracts/hwax-agent/install-report.schema.json (HWAXAgentInstallReport)
 *   - contracts/hwax-agent/audit-event.schema.json    (HWAXAgentAuditEvent)
 *
 * The wire format uses snake_case keys, so these interfaces use snake_case too —
 * they describe JSON payloads on the HTTP boundary, NOT the Rust crate's internal
 * field naming (which renames e.g. Package.type -> Package.kind via serde).
 *
 * Every object in the schemas declares `additionalProperties: false`. To keep
 * these types honest, NO interface here adds a key the schema does not define.
 * For the runtime guarantee, validate with ./validate (ajv enforces
 * additionalProperties:false against real payloads — types alone cannot).
 *
 * Optionality: a property that is NOT in a schema's `required` array is marked
 * optional (`?`) here. A property typed `["string","null"]` in the schema is
 * modelled as `T | null` (the key may be present-and-null), and where it is also
 * not required it is `T | null | undefined` (via `?`).
 */

/* ───────────────────────── manifest.schema.json ───────────────────────── */

/** Package payload type (`Package.type`). */
export type PackageType = 'zip' | 'exe' | 'msi' | 'msix';

/** Program visibility scope (`Program.visibility`). */
export type Visibility = 'private' | 'team' | 'department' | 'company';

/**
 * Installer payload descriptor. The agent GETs `url` (origin must be
 * allow-listed) and MUST verify `sha256` over the downloaded bytes before
 * extract/execute.
 */
export interface Package {
  /** Wire key is `type`. */
  type: PackageType;
  /**
   * Absolute URL the agent GETs. In Phase 1 this is the HEAXHub 302-redirect
   * endpoint `GET /api/v1/installers/{id}/download`. JSON Schema `format: uri`.
   */
  url: string;
  /** Lowercase hex SHA-256 of the installer bytes. Pattern `^[a-f0-9]{64}$`. */
  sha256: string;
  /** `minimum: 0`. */
  size_bytes?: number;
}

/**
 * The single runnable entry point of an installed program. The agent executes
 * ONLY this whitelisted relative path — never an arbitrary or user-supplied exe.
 */
export interface Entry {
  /** Relative path inside the installed program root, e.g. `bin/HwaxDemo.exe`. */
  executable: string;
  /**
   * Argument tokens. Tokens like `{workspace}` / `{user_id}` are substituted at
   * launch time by the agent — the agent supplies no free-form user args.
   */
  args_template?: string[];
  working_dir?: string;
}

/** Install/runtime requirements gate. */
export interface Requirements {
  /** Schema default `false`. */
  requires_admin?: boolean;
  /** Minimum Windows build, e.g. `10.0.19045` or `11.0.22000`. */
  min_windows?: string;
  /** Other program ids that must be installed first. `uniqueItems: true`. */
  depends_on?: string[];
}

/** Optional post-install health probe (whitelisted executable + args). */
export interface PostInstallCheck {
  executable: string;
  args?: string[];
  expected_stdout_regex?: string;
}

/** Lifecycle hooks for an install. */
export interface Lifecycle {
  post_install_check?: PostInstallCheck;
  /** Schema default `true`. */
  rollback_on_failure?: boolean;
}

/** Presentational hints for the agent tile grid / tray. */
export interface UiHints {
  /** JSON Schema `format: uri`. */
  icon_url?: string;
  /** `#RRGGBB`. Pattern `^#[0-9a-fA-F]{6}$`. */
  color_accent?: string;
  /** Schema default `false`. */
  show_in_tray?: boolean;
}

/** One installable Windows GUI module (a tile in the agent grid). */
export interface Program {
  /**
   * Stable App slug — matches `App.id` in HEAXHub. Used as the on-disk module
   * directory name and registry key suffix. Pattern `^[a-z0-9][a-z0-9_-]*$`,
   * length 1..=64.
   */
  id: string;
  /** Human-readable display name. Length 1..=128. */
  name: string;
  /** SemVer. Pattern `^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$`. */
  version: string;
  /** `maxLength: 1024`. */
  description?: string;
  /** `maxLength: 64`. */
  category?: string;
  /** `format: date-time`. */
  released_at?: string;
  package: Package;
  entry: Entry;
  requirements?: Requirements;
  lifecycle?: Lifecycle;
  ui?: UiHints;
  /** Item `maxLength: 32`, `uniqueItems: true`. */
  tags?: string[];
  visibility?: Visibility;
}

/**
 * Top-level program catalog — the `GET /api/v1/launcher-agents/manifest`
 * response body (HWAXAgentManifest).
 */
export interface Manifest {
  /** `const: 1`. Bumped only on breaking changes. */
  schema_version: 1;
  /** Server snapshot time. `format: date-time`. The agent caches keyed by this. */
  generated_at: string;
  programs: Program[];
}

/* ─────────────────────── install-report.schema.json ─────────────────────── */

/**
 * Terminal status of one install attempt (HWAXAgentInstallReport.status).
 * `partial` = installer exited 0 but post_install_check failed and rollback was
 * not requested. Intentionally DISJOINT from {@link AuditKind}.
 */
export type InstallStatus = 'success' | 'failed' | 'rolled_back' | 'partial';

/**
 * Per-attempt install outcome — the `POST /api/v1/launcher-agents/installs`
 * body (HWAXAgentInstallReport).
 */
export interface InstallReport {
  /** `WindowsAgent.id` (UUID) — must match the `sub` of the access token. */
  agent_id: string;
  /** App slug. Pattern `^[a-z0-9][a-z0-9_-]*$`, length 1..=64. */
  app_id: string;
  /** SemVer. */
  version: string;
  status: InstallStatus;
  /** Schema type `["integer","null"]`, range i32. May be present-and-null. */
  exit_code?: number | null;
  /** `format: date-time`. */
  started_at: string;
  /** `format: date-time`. */
  finished_at: string;
  /**
   * True iff the agent computed SHA-256 over the bytes and it matched the
   * manifest. The hub treats `false` as a hard failure.
   */
  sha256_verified?: boolean;
  /** Short error summary. `maxLength: 2048`. Schema type `["string","null"]`. */
  error?: string | null;
  /** Tail of installer stdout/stderr. `maxLength: 16384`. `["string","null"]`. */
  log_excerpt?: string | null;
  /** Version rolled back TO (meaningful only when `status=rolled_back`). SemVer. */
  previous_version?: string | null;
}

/* ──────────────────────── audit-event.schema.json ──────────────────────── */

/**
 * Audit event class (HWAXAgentAuditEvent.kind). Intentionally DISJOINT from
 * {@link InstallStatus}: kind classifies the event, status classifies the
 * terminal outcome of one install attempt.
 */
export type AuditKind =
  | 'enrollment'
  | 'install'
  | 'uninstall'
  | 'run'
  | 'stop'
  | 'rollback'
  | 'av_block'
  | 'sha256_mismatch'
  | 'download_failed'
  | 'policy_denied'
  | 'heartbeat';

/** Audit severity (HWAXAgentAuditEvent.severity). */
export type Severity = 'info' | 'warn' | 'error';

/**
 * Optional client metadata block. `additionalProperties: false` — only these
 * four keys are permitted.
 */
export interface ClientMeta {
  /** e.g. `"windows"`. */
  os?: string;
  /** e.g. `"10.0.19045"`. */
  os_version?: string;
  /** SemVer. Pattern `^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$`. */
  agent_version?: string;
  /** `maxLength: 256`. */
  hostname?: string;
}

/**
 * Single audit event — the `POST /api/v1/launcher-agents/audit` body
 * (HWAXAgentAuditEvent).
 *
 * NOTE: the `payload` schema is `{ "type": "object" }` WITHOUT
 * `additionalProperties: false`, so it accepts any JSON object of free-form,
 * kind-specific context. We model it as an open record.
 */
export interface AuditEvent {
  /** `format: uuid`. */
  agent_id: string;
  kind: AuditKind;
  /** App slug or null. Pattern `^[a-z0-9][a-z0-9_-]*$`, `maxLength: 64`. */
  app_id?: string | null;
  /** SemVer or null. */
  version?: string | null;
  /** `format: date-time`. */
  occurred_at: string;
  severity: Severity;
  /** Free-form object; the hub stores as JSONB and does not interpret it. */
  payload?: Record<string, unknown>;
  client_meta?: ClientMeta;
}
