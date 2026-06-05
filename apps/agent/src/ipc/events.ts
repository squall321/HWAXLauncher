/**
 * Typed `listen()` wrappers for the Rust → JS events (catalog EVENTS section).
 *
 * Each helper returns the `UnlistenFn` promise from `@tauri-apps/api/event`;
 * callers (hooks) must await it and invoke it on cleanup to avoid leaking
 * listeners across panel remounts.
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  InstallProgressEvent,
  StateChangedEvent,
  SyncDoneEvent,
} from './types';

/** Stable event-name constants — single source for emit/listen parity. */
export const EVENT = {
  installProgress: 'install:progress',
  syncDone: 'sync:done',
  stateChanged: 'state:changed',
} as const;

/** `install:progress` — per-phase percent during an install (v2 §10). */
export const onInstallProgress = (
  handler: (payload: InstallProgressEvent) => void,
): Promise<UnlistenFn> =>
  listen<InstallProgressEvent>(EVENT.installProgress, (e) => handler(e.payload));

/** `sync:done` — emitted after a manifest sync completes. */
export const onSyncDone = (
  handler: (payload: SyncDoneEvent) => void,
): Promise<UnlistenFn> =>
  listen<SyncDoneEvent>(EVENT.syncDone, (e) => handler(e.payload));

/** `state:changed` — a module transitioned in the lifecycle state machine. */
export const onStateChanged = (
  handler: (payload: StateChangedEvent) => void,
): Promise<UnlistenFn> =>
  listen<StateChangedEvent>(EVENT.stateChanged, (e) => handler(e.payload));
