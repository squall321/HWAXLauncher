import { useCallback, useEffect, useState } from 'react';
import { agentStatus } from '../ipc/commands';
import { onStateChanged, onSyncDone } from '../ipc/events';
import type { AgentStatus } from '../ipc/types';

/**
 * Polls + reactively refreshes the tray status header.
 * Re-fetches on `sync:done` and any `state:changed` so the dot color, sync
 * timestamp and error count stay live without busy-polling.
 */
export function useAgentStatus(pollMs = 15_000) {
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const s = await agentStatus();
      setStatus(s);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(refresh, pollMs);

    const unlisteners: Promise<() => void>[] = [
      onSyncDone(() => void refresh()),
      onStateChanged(() => void refresh()),
    ];

    return () => {
      window.clearInterval(timer);
      unlisteners.forEach((p) => void p.then((un) => un()));
    };
  }, [refresh, pollMs]);

  return { status, loading, error, refresh };
}
