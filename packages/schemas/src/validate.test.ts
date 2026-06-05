/**
 * Validates one GOOD and one BAD sample per schema against the real, vendored
 * contracts in contracts/hwax-agent/. This is the drift guard: if the canonical
 * schema changes shape, these assertions (and the hand-written types they mirror)
 * must be updated together.
 */

import { describe, it, expect } from 'vitest';

import {
  validateManifest,
  validateInstallReport,
  validateAuditEvent,
  formatErrors,
} from './validate.js';
import type { Manifest, InstallReport, AuditEvent } from './types.js';

const AGENT_ID = '550e8400-e29b-41d4-a716-446655440000';

/* ───────────────────────────── manifest ───────────────────────────── */

const goodManifest: Manifest = {
  schema_version: 1,
  generated_at: '2026-06-06T09:00:00Z',
  programs: [
    {
      id: 'koo-preprocessor',
      name: 'Koo Preprocessor',
      version: '1.4.2',
      description: 'Mesh preprocessing GUI.',
      category: 'cae',
      released_at: '2026-06-01T00:00:00Z',
      package: {
        type: 'zip',
        url: 'https://heaxhub.local/api/v1/installers/abc/download',
        sha256: 'a'.repeat(64),
        size_bytes: 10485760,
      },
      entry: {
        executable: 'bin/KooPre.exe',
        args_template: ['--workspace', '{workspace}'],
        working_dir: 'bin',
      },
      requirements: { requires_admin: false, min_windows: '10.0.19045' },
      lifecycle: {
        post_install_check: {
          executable: 'bin/KooPre.exe',
          args: ['--version'],
          expected_stdout_regex: '^1\\.4\\.2',
        },
        rollback_on_failure: true,
      },
      ui: { color_accent: '#f59e0b', show_in_tray: true },
      tags: ['cae', 'mesh'],
      visibility: 'company',
    },
  ],
};

describe('validateManifest', () => {
  it('accepts a well-formed manifest', () => {
    const ok = validateManifest(goodManifest);
    expect(ok, formatErrors(validateManifest.errors)).toBe(true);
  });

  it('rejects a manifest whose program carries an unknown key', () => {
    // additionalProperties:false — `rank` is not in the Program schema.
    const bad = {
      schema_version: 1,
      generated_at: '2026-06-06T09:00:00Z',
      programs: [
        {
          id: 'koo-preprocessor',
          name: 'Koo Preprocessor',
          version: '1.4.2',
          rank: 3,
          package: { type: 'zip', url: 'https://heaxhub.local/x', sha256: 'a'.repeat(64) },
          entry: { executable: 'bin/KooPre.exe' },
        },
      ],
    };
    expect(validateManifest(bad)).toBe(false);
    expect(formatErrors(validateManifest.errors)).toContain('rank');
  });

  it('rejects a bad sha256 / package type / version', () => {
    const bad = {
      schema_version: 1,
      generated_at: '2026-06-06T09:00:00Z',
      programs: [
        {
          id: 'koo-preprocessor',
          name: 'Koo Preprocessor',
          version: 'not-semver',
          package: { type: 'tarball', url: 'https://heaxhub.local/x', sha256: 'XYZ' },
          entry: { executable: 'bin/KooPre.exe' },
        },
      ],
    };
    expect(validateManifest(bad)).toBe(false);
  });
});

/* ──────────────────────────── install report ──────────────────────────── */

const goodReport: InstallReport = {
  agent_id: AGENT_ID,
  app_id: 'koo-preprocessor',
  version: '1.4.2',
  status: 'success',
  exit_code: 0,
  started_at: '2026-06-06T09:00:00Z',
  finished_at: '2026-06-06T09:01:30Z',
  sha256_verified: true,
  error: null,
  log_excerpt: null,
  previous_version: null,
};

describe('validateInstallReport', () => {
  it('accepts a well-formed success report', () => {
    const ok = validateInstallReport(goodReport);
    expect(ok, formatErrors(validateInstallReport.errors)).toBe(true);
  });

  it('rejects an invalid status enum value', () => {
    const bad = { ...goodReport, status: 'done' };
    expect(validateInstallReport(bad)).toBe(false);
    expect(formatErrors(validateInstallReport.errors)).toContain('status');
  });

  it('rejects a non-uuid agent_id', () => {
    const bad = { ...goodReport, agent_id: 'not-a-uuid' };
    expect(validateInstallReport(bad)).toBe(false);
  });
});

/* ──────────────────────────── audit event ──────────────────────────── */

const goodAudit: AuditEvent = {
  agent_id: AGENT_ID,
  kind: 'sha256_mismatch',
  app_id: 'koo-preprocessor',
  version: '1.4.2',
  occurred_at: '2026-06-06T09:00:30Z',
  severity: 'error',
  payload: { expected: 'a'.repeat(64), actual: 'b'.repeat(64) },
  client_meta: {
    os: 'windows',
    os_version: '10.0.19045',
    agent_version: '0.1.0',
    hostname: 'DESKTOP-01',
  },
};

describe('validateAuditEvent', () => {
  it('accepts a well-formed audit event', () => {
    const ok = validateAuditEvent(goodAudit);
    expect(ok, formatErrors(validateAuditEvent.errors)).toBe(true);
  });

  it('rejects an unknown kind', () => {
    const bad = { ...goodAudit, kind: 'exploded' };
    expect(validateAuditEvent(bad)).toBe(false);
    expect(formatErrors(validateAuditEvent.errors)).toContain('kind');
  });

  it('rejects an unknown key in client_meta (additionalProperties:false)', () => {
    const bad = {
      ...goodAudit,
      client_meta: { ...goodAudit.client_meta, locale: 'ko-KR' },
    };
    expect(validateAuditEvent(bad)).toBe(false);
  });
});
