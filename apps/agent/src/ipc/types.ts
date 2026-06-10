/**
 * TypeScript mirror of the Tauri IPC command catalog.
 *
 * Field names are **snake_case** on purpose: they cross the `#[tauri::command]`
 * boundary verbatim (the Rust side derives serde defaults, no rename), so the
 * shapes here must match the catalog byte-for-byte. Do not "camelCase" these.
 *
 * These types are the authoritative wire format for the React app; they mirror
 * the IPC catalog in the workstream brief, which in turn mirrors
 * `crates/hwax-core` (state machine, config, contract models).
 */

/* ───────────────────────── Module lifecycle ───────────────────────── */

/** 11 resting/transition states + 3 failure states (v2 plan §6). */
export type ModuleState =
  | 'idle'
  | 'checking'
  | 'installed'
  | 'outdated'
  | 'not_installed'
  | 'downloading'
  | 'verifying'
  | 'extracting'
  | 'swapping'
  | 'running'
  | 'stopped'
  | 'failed'
  | 'rolling_back'
  | 'rolled_back';

/** Phases surfaced through the `install:progress` event (v2 §10). */
export type InstallPhase = 'download' | 'verify' | 'extract' | 'check' | 'swap';

/** Tray status dot color (v2 §4.1): green ok / yellow warn / red error. */
export type StatusColor = 'green' | 'yellow' | 'red';

/* ───────────────────────── Status & pairing ───────────────────────── */

export interface AgentStatus {
  agent_id: string | null;
  server: string | null;
  paired: boolean;
  last_sync: string | null;
  module_count: number;
  error_count: number;
  status_color: StatusColor;
}

export interface PairingInfo {
  /** Deep-link to the HEAXHub admin agents console (`/admin/agents`), where an
   *  operator registers this PC and issues the single-use enrollment token. */
  url: string;
}

/* ───────────────────────── Modules ───────────────────────── */

export interface ModuleView {
  id: string;
  name: string;
  current_version: string | null;
  latest_version: string | null;
  state: ModuleState;
  show_in_tray: boolean;
  /** Per-module accent (manifest `ui.color_accent`), or null ⇒ use theme. */
  color_accent: string | null;
  category: string | null;
}

export interface ModuleHistoryEntry {
  version: string;
  installed_at: string;
}

export interface ModuleDetail {
  id: string;
  name: string;
  description: string | null;
  category: string | null;
  current_version: string | null;
  latest_version: string | null;
  state: ModuleState;
  history: ModuleHistoryEntry[];
  requires_admin: boolean;
}

export interface SyncResult {
  changed: boolean;
  modules: ModuleView[];
}

export interface RunHandle {
  pid: number;
  id: string;
}

/* ───────────────────────── Config ───────────────────────── */

export interface AgentConfig {
  server: string;
  agent_id: string;
  auto_update: boolean;
  start_on_boot: boolean;
  log_level: string;
  allowed_origins: string[];
  keep_last_n_versions: number;
  sync_interval_min: number;
  channel: string;
  proxy: string | null;
  telemetry_anonymous: boolean;
}

/* ───────────────────────── Diagnostics ───────────────────────── */

export interface HealthReport {
  server_reachable: boolean;
  disk_free_bytes: number;
  write_ok: boolean;
}

/** `check_update()` result — null when the agent is already current. */
export interface UpdateInfo {
  version: string;
  current_version: string;
  notes: string | null;
}

/* ───────────────────────── Event payloads ───────────────────────── */

export interface InstallProgressEvent {
  id: string;
  phase: InstallPhase;
  /** 0..=100 */
  percent: number;
}

export interface SyncDoneEvent {
  changed: boolean;
}

export interface StateChangedEvent {
  id: string;
  state: ModuleState;
}
