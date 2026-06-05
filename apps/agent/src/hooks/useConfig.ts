import { useCallback, useEffect, useState } from 'react';
import { getConfig, updateConfig } from '../ipc/commands';
import type { AgentConfig } from '../ipc/types';

/**
 * Loads the agent config and exposes a `save(patch)` that writes through the
 * `update_config` command and adopts the server-returned canonical config.
 *
 * Note: `server` is deliberately not exposed as patchable here — the Settings
 * panel renders it read-only (re-pairing only, an AV-evasion requirement).
 */
export function useConfig() {
  const [config, setConfig] = useState<AgentConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setConfig(await getConfig());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  /** Persist a partial patch; `server` is stripped defensively. */
  const save = useCallback(async (patch: Partial<AgentConfig>) => {
    setSaving(true);
    try {
      // Never let the read-only server address leave the UI.
      const { server: _server, ...safe } = patch;
      void _server;
      const next = await updateConfig(safe);
      setConfig(next);
      setError(null);
      return next;
    } catch (e) {
      setError(String(e));
      throw e;
    } finally {
      setSaving(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { config, loading, saving, error, refresh, save };
}
