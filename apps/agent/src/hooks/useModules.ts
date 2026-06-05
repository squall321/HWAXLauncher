import { useCallback, useEffect, useState } from 'react';
import { listModules } from '../ipc/commands';
import { onStateChanged, onSyncDone } from '../ipc/events';
import type { ModuleState, ModuleView } from '../ipc/types';

/**
 * Loads the module list and keeps it live.
 * - `sync:done` ⇒ full reload (versions may have changed).
 * - `state:changed` ⇒ patch the single module's state in place so transient
 *   transitions (downloading → verifying → …) update without a round-trip.
 */
export function useModules() {
  const [modules, setModules] = useState<ModuleView[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await listModules();
      setModules(list);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const patchState = useCallback((id: string, state: ModuleState) => {
    setModules((prev) =>
      prev.map((m) => (m.id === id ? { ...m, state } : m)),
    );
  }, []);

  useEffect(() => {
    void refresh();
    const unlisteners: Promise<() => void>[] = [
      onSyncDone(() => void refresh()),
      onStateChanged((p) => patchState(p.id, p.state)),
    ];
    return () => {
      unlisteners.forEach((pr) => void pr.then((un) => un()));
    };
  }, [refresh, patchState]);

  return { modules, loading, error, refresh, patchState };
}
