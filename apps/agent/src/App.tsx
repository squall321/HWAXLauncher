import { useState } from 'react';
import { Loader2 } from 'lucide-react';
import { useAgentStatus, useSync } from './hooks';
import { ModuleDetail, Pairing, Settings, TrayMain } from './panels';
import type { AgentStatus } from './ipc/types';

/**
 * Single-window router. The agent runs one frameless tray panel and swaps the
 * whole view between the four screens (v2 §4). No URL routing — this is a
 * tray utility, not a multi-page app.
 */
type Route =
  | { name: 'tray' }
  | { name: 'detail'; id: string }
  | { name: 'settings' }
  | { name: 'pairing' };

export default function App() {
  const { status, loading, refresh } = useAgentStatus();
  const { sync, syncing } = useSync();
  const [route, setRoute] = useState<Route>({ name: 'tray' });

  // First load: decide between the pairing wizard and the main panel.
  if (loading) {
    return (
      <div className="flex h-full items-center justify-center bg-hwax-bg text-hwax-muted">
        <Loader2 size={18} className="animate-spin" />
      </div>
    );
  }

  // Not paired yet (or user chose "re-pair") → enrollment wizard.
  const needsPairing = !status?.paired || route.name === 'pairing';
  if (needsPairing) {
    return (
      <Pairing
        onPaired={(s: AgentStatus) => {
          void refresh();
          // Adopt the returned status implicitly via refresh; go to the list.
          void s;
          setRoute({ name: 'tray' });
        }}
      />
    );
  }

  switch (route.name) {
    case 'detail':
      return <ModuleDetail id={route.id} onBack={() => setRoute({ name: 'tray' })} />;
    case 'settings':
      return (
        <Settings
          onBack={() => setRoute({ name: 'tray' })}
          onRepair={() => setRoute({ name: 'pairing' })}
        />
      );
    case 'tray':
    default:
      return (
        <TrayMain
          status={status}
          onSync={sync}
          syncing={syncing}
          onOpenSettings={() => setRoute({ name: 'settings' })}
          onOpenDetail={(id) => setRoute({ name: 'detail', id })}
        />
      );
  }
}
