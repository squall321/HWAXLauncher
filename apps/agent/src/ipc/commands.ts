/**
 * Typed `invoke()` wrappers for every command in the IPC catalog.
 *
 * Each function is a thin, named pass-through to the Rust `#[tauri::command]`
 * of the same name with the exact argument shape the catalog specifies. UI
 * code should call these instead of `invoke` directly so the command names and
 * payloads live in one auditable place.
 */
import { invoke } from '@tauri-apps/api/core';
import type {
  AgentConfig,
  AgentStatus,
  HealthReport,
  ModuleDetail,
  ModuleView,
  PairingInfo,
  RunHandle,
  SyncResult,
  UpdateInfo,
} from './types';

/* ───────────────────────── Status & pairing ───────────────────────── */

export const agentStatus = (): Promise<AgentStatus> => invoke('agent_status');

export const startPairing = (): Promise<PairingInfo> => invoke('start_pairing');

export const completePairing = (enrollment_token: string): Promise<AgentStatus> =>
  invoke('complete_pairing', { enrollment_token });

/* ───────────────────────── Sync & modules ───────────────────────── */

export const syncManifest = (): Promise<SyncResult> => invoke('sync_manifest');

export const listModules = (): Promise<ModuleView[]> => invoke('list_modules');

export const moduleDetail = (id: string): Promise<ModuleDetail> =>
  invoke('module_detail', { id });

/* ───────────────────────── Install lifecycle ───────────────────────── */

/** Progress arrives out-of-band via the `install:progress` event. */
export const installModule = (id: string, version: string): Promise<void> =>
  invoke('install_module', { id, version });

export const cancelInstall = (id: string): Promise<void> =>
  invoke('cancel_install', { id });

export const runModule = (id: string): Promise<RunHandle> =>
  invoke('run_module', { id });

export const stopModule = (id: string): Promise<void> =>
  invoke('stop_module', { id });

/** `target` null/undefined ⇒ roll back to the previous version. */
export const rollbackModule = (id: string, target?: string | null): Promise<void> =>
  invoke('rollback_module', { id, target: target ?? null });

export const uninstallModule = (id: string): Promise<void> =>
  invoke('uninstall_module', { id });

/* ───────────────────────── Logs ───────────────────────── */

/** Opens the log folder/file in Explorer (id null ⇒ the agent's own log). */
export const openLog = (id?: string | null): Promise<void> =>
  invoke('open_log', { id: id ?? null });

export const tailLog = (lines: number, id?: string | null): Promise<string> =>
  invoke('tail_log', { id: id ?? null, lines });

/* ───────────────────────── Config ───────────────────────── */

export const getConfig = (): Promise<AgentConfig> => invoke('get_config');

/**
 * Patch config. The server address is intentionally never patched from the UI
 * (AV-evasion: changing it is re-pairing only). The Settings panel renders it
 * read-only and never includes `server` in the patch.
 */
export const updateConfig = (patch: Partial<AgentConfig>): Promise<AgentConfig> =>
  invoke('update_config', { patch });

/* ───────────────────────── Diagnostics & lifecycle ───────────────────────── */

export const healthCheck = (): Promise<HealthReport> => invoke('health_check');

/** Returns the absolute path to the produced diagnostic zip. */
export const makeDump = (): Promise<string> => invoke('make_dump');

export const clearCache = (): Promise<void> => invoke('clear_cache');

export const quit = (): Promise<void> => invoke('quit');

/* ───────────────────────── Agent self-update (v2 §18) ───────────────────────── */

/** Returns the available signed update, or null when already current. */
export const checkUpdate = (): Promise<UpdateInfo | null> => invoke('check_update');

/** Downloads + applies the update, then the agent restarts (never resolves). */
export const installUpdate = (): Promise<void> => invoke('install_update');
