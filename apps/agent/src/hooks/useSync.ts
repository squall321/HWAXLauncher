import { useCallback, useState } from 'react';
import { syncManifest } from '../ipc/commands';
import type { SyncResult } from '../ipc/types';

/**
 * Imperative "sync now" trigger for the tray + settings panels.
 * The Rust side also emits `sync:done`, which other hooks react to; this hook
 * just exposes the manual button state and the immediate result.
 */
export function useSync() {
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastResult, setLastResult] = useState<SyncResult | null>(null);

  const sync = useCallback(async () => {
    setSyncing(true);
    setError(null);
    try {
      const result = await syncManifest();
      setLastResult(result);
      return result;
    } catch (e) {
      setError(String(e));
      throw e;
    } finally {
      setSyncing(false);
    }
  }, []);

  return { sync, syncing, error, lastResult };
}
