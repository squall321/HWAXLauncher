import { useMemo, useState } from 'react';
import {
  FolderOpen,
  Loader2,
  RefreshCw,
  Search,
  Settings as SettingsIcon,
} from 'lucide-react';
import {
  cancelInstall,
  installModule,
  openLog,
  runModule,
  stopModule,
} from '../ipc/commands';
import type { AgentStatus, ModuleView } from '../ipc/types';
import { useInstallProgress, useModules } from '../hooks';
import { ModuleCard } from '../components/ModuleCard';
import { StatusDot } from '../components/StatusDot';
import { Button } from '../components/Button';
import { useToast } from '../components/Toast';

interface TrayMainProps {
  status: AgentStatus | null;
  onSync: () => Promise<unknown>;
  syncing: boolean;
  onOpenSettings: () => void;
  onOpenDetail: (id: string) => void;
}

function relativeSync(iso: string | null): string {
  if (!iso) return '동기화 안 됨';
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return '동기화 안 됨';
  const secs = Math.max(0, Math.floor((Date.now() - then) / 1000));
  if (secs < 60) return `동기화 ${secs}초 전`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `동기화 ${mins}분 전`;
  return `동기화 ${Math.floor(mins / 60)}시간 전`;
}

/**
 * Main tray panel — compact, searchable module list with inline actions
 * (v2 §4.2, 480×640). Header carries the status dot + relative sync time.
 */
export function TrayMain({
  status,
  onSync,
  syncing,
  onOpenSettings,
  onOpenDetail,
}: TrayMainProps) {
  const { modules, loading, refresh } = useModules();
  const { byId } = useInstallProgress();
  const toast = useToast();
  const [query, setQuery] = useState('');
  const [busyId, setBusyId] = useState<string | null>(null);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return modules;
    return modules.filter(
      (m) =>
        m.name.toLowerCase().includes(q) ||
        m.id.toLowerCase().includes(q) ||
        (m.category ?? '').toLowerCase().includes(q),
    );
  }, [modules, query]);

  // Wrap a command so the originating card disables while it resolves; module
  // state then flows back via the `state:changed` event (useModules patches it).
  const withBusy =
    (id: string, fn: () => Promise<unknown>, successMsg?: string) => async () => {
      setBusyId(id);
      try {
        await fn();
        if (successMsg) toast.push('success', successMsg);
      } catch (e) {
        // Surface the IPC error inline and re-read the authoritative state.
        toast.push('error', String(e));
        void refresh();
      } finally {
        setBusyId(null);
      }
    };

  const headerColor = status?.status_color ?? 'green';

  return (
    <div className="flex h-full flex-col bg-hwax-bg">
      {/* ── Status header ───────────────────────────────────────────── */}
      <header className="flex items-center gap-2 border-b border-hwax-border px-4 py-2.5">
        <StatusDot color={headerColor} pulse={syncing} title={`상태: ${headerColor}`} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold text-hwax-text">HWAX Agent</div>
          <div className="truncate text-[11px] text-hwax-muted">
            {status?.paired
              ? `${relativeSync(status.last_sync)} · 모듈 ${status.module_count}개`
              : '페어링되지 않음'}
            {status && status.error_count > 0 && (
              <span className="text-status-red"> · 오류 {status.error_count}</span>
            )}
          </div>
        </div>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => void onSync()}
          disabled={syncing}
          title="지금 동기화"
        >
          {syncing ? (
            <Loader2 size={14} className="animate-spin" />
          ) : (
            <RefreshCw size={14} />
          )}
        </Button>
        <Button size="sm" variant="ghost" onClick={() => void openLog(null)} title="로그 폴더 열기">
          <FolderOpen size={14} />
        </Button>
        <Button size="sm" variant="ghost" onClick={onOpenSettings} title="설정">
          <SettingsIcon size={14} />
        </Button>
      </header>

      {/* ── Search ──────────────────────────────────────────────────── */}
      <div className="px-4 pb-1 pt-3">
        <div className="flex items-center gap-2 rounded-md border border-hwax-border bg-hwax-elevated px-2.5">
          <Search size={14} className="text-hwax-muted" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="검색"
            className="h-8 w-full bg-transparent text-sm text-hwax-text placeholder:text-hwax-muted/60 focus:outline-none"
          />
        </div>
      </div>

      {/* ── Module list ─────────────────────────────────────────────── */}
      <div className="flex-1 space-y-2 overflow-y-auto px-4 py-2">
        {loading ? (
          <div className="flex items-center justify-center gap-2 py-10 text-sm text-hwax-muted">
            <Loader2 size={16} className="animate-spin" /> 모듈을 불러오는 중…
          </div>
        ) : filtered.length === 0 ? (
          <div className="py-10 text-center text-sm text-hwax-muted">
            {query ? '검색 결과가 없습니다.' : '표시할 모듈이 없습니다.'}
          </div>
        ) : (
          filtered.map((m: ModuleView) => (
            <ModuleCard
              key={m.id}
              module={m}
              progress={byId[m.id] ?? null}
              busy={busyId === m.id}
              onInstall={(mod) =>
                void withBusy(
                  mod.id,
                  () => installModule(mod.id, mod.latest_version ?? ''),
                  `${mod.name} ${mod.latest_version ?? ''} 설치 완료`,
                )()
              }
              onUpdate={(mod) =>
                void withBusy(
                  mod.id,
                  () => installModule(mod.id, mod.latest_version ?? ''),
                  `${mod.name} ${mod.latest_version ?? ''} 업데이트 완료`,
                )()
              }
              onRun={(mod) => void withBusy(mod.id, () => runModule(mod.id))()}
              onStop={(mod) => void withBusy(mod.id, () => stopModule(mod.id))()}
              onCancel={(mod) => void cancelInstall(mod.id)}
              onDetail={(mod) => onOpenDetail(mod.id)}
              onLogs={(mod) => void openLog(mod.id)}
            />
          ))
        )}
      </div>
    </div>
  );
}
