import { useCallback, useEffect, useState } from 'react';
import {
  ArrowLeft,
  FileText,
  Loader2,
  Play,
  RotateCcw,
  ShieldAlert,
  Square,
  Trash2,
} from 'lucide-react';
import {
  moduleDetail,
  openLog,
  rollbackModule,
  runModule,
  stopModule,
  uninstallModule,
} from '../ipc/commands';
import type { ModuleDetail as ModuleDetailData } from '../ipc/types';
import { onStateChanged } from '../ipc/events';
import { Button } from '../components/Button';
import { StateBadge } from '../components/Badge';
import { useToast } from '../components/Toast';

interface ModuleDetailProps {
  id: string;
  onBack: () => void;
}

function fmtDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toISOString().slice(0, 10);
}

/** Module detail panel: metadata, version history + rollback (v2 §4.3). */
export function ModuleDetail({ id, onBack }: ModuleDetailProps) {
  const [detail, setDetail] = useState<ModuleDetailData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const load = useCallback(async () => {
    try {
      setDetail(await moduleDetail(id));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [id]);

  useEffect(() => {
    void load();
    // Re-fetch when this module transitions (e.g. after a rollback completes).
    const p = onStateChanged((evt) => {
      if (evt.id === id) void load();
    });
    return () => void p.then((un) => un());
  }, [id, load]);

  const run = async (fn: () => Promise<unknown>, successMsg?: string) => {
    setBusy(true);
    try {
      await fn();
      if (successMsg) toast.push('success', successMsg);
      await load();
    } catch (e) {
      setError(String(e));
      toast.push('error', String(e));
    } finally {
      setBusy(false);
    }
  };

  if (!detail) {
    return (
      <div className="flex h-full flex-col bg-hwax-bg">
        <DetailHeader name="" onBack={onBack} />
        <div className="flex flex-1 items-center justify-center gap-2 text-sm text-hwax-muted">
          {error ? (
            <span className="text-status-red">{error}</span>
          ) : (
            <>
              <Loader2 size={16} className="animate-spin" /> 불러오는 중…
            </>
          )}
        </div>
      </div>
    );
  }

  const current = detail.current_version;
  const running = detail.state === 'running';
  const installed = current !== null;

  return (
    <div className="flex h-full flex-col bg-hwax-bg">
      <DetailHeader name={detail.name} onBack={onBack} />

      <div className="flex-1 overflow-y-auto px-5 py-4">
        {/* Metadata */}
        <div className="space-y-1.5 text-sm">
          <Row label="현재 버전">
            {current ? (
              <span className="font-mono text-hwax-text">{current}</span>
            ) : (
              <span className="text-hwax-muted">미설치</span>
            )}
          </Row>
          <Row label="서버 최신">
            <span className="font-mono text-hwax-text">
              {detail.latest_version ?? '—'}
            </span>
          </Row>
          {detail.category && <Row label="카테고리">{detail.category}</Row>}
          <Row label="상태">
            <StateBadge state={detail.state} />
          </Row>
          <Row label="요구 권한">
            {detail.requires_admin ? (
              <span className="inline-flex items-center gap-1 text-status-yellow">
                <ShieldAlert size={13} /> 관리자
              </span>
            ) : (
              <span className="text-hwax-muted">사용자 (관리자 X)</span>
            )}
          </Row>
        </div>

        {detail.description && (
          <p
            data-selectable
            className="mt-3 rounded-md border border-hwax-border bg-hwax-elevated p-3 text-xs leading-relaxed text-hwax-muted"
          >
            {detail.description}
          </p>
        )}

        {/* Version history + rollback */}
        <h2 className="mb-2 mt-5 text-xs font-medium uppercase tracking-wide text-hwax-muted">
          변경 이력
        </h2>
        <ul className="divide-y divide-hwax-border overflow-hidden rounded-md border border-hwax-border">
          {detail.history.length === 0 ? (
            <li className="px-3 py-3 text-xs text-hwax-muted">이력이 없습니다.</li>
          ) : (
            detail.history.map((h) => {
              const isCurrent = h.version === current;
              return (
                <li
                  key={h.version}
                  className="flex items-center justify-between gap-2 bg-hwax-elevated px-3 py-2.5"
                >
                  <div className="flex items-center gap-2">
                    <span className="text-hwax-muted">◇</span>
                    <span className="font-mono text-sm text-hwax-text">{h.version}</span>
                    {isCurrent && (
                      <span className="text-[10px] font-medium text-hwax-accent">현재</span>
                    )}
                  </div>
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-xs text-hwax-muted">
                      {fmtDate(h.installed_at)}
                    </span>
                    {!isCurrent && installed && (
                      <Button
                        size="sm"
                        variant="danger"
                        disabled={busy}
                        onClick={() =>
                          void run(
                            () => rollbackModule(id, h.version),
                            `${h.version}(으)로 롤백했습니다`,
                          )
                        }
                      >
                        <RotateCcw size={12} /> 롤백
                      </Button>
                    )}
                  </div>
                </li>
              );
            })
          )}
        </ul>
      </div>

      {error && (
        <p className="mx-5 mb-2 rounded-md border border-status-red/30 bg-status-red/10 px-3 py-2 text-xs text-status-red">
          {error}
        </p>
      )}

      {/* Footer actions */}
      <footer className="flex flex-wrap items-center gap-1.5 border-t border-hwax-border px-5 py-3">
        {installed &&
          (running ? (
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={() => void run(() => stopModule(id), `${detail.name} 중지됨`)}
            >
              <Square size={13} /> 중지
            </Button>
          ) : (
            <Button
              size="sm"
              variant="primary"
              disabled={busy}
              onClick={() => void run(() => runModule(id), `${detail.name} 실행됨`)}
            >
              <Play size={13} /> 실행
            </Button>
          ))}
        <Button size="sm" variant="ghost" onClick={() => void openLog(id)}>
          <FileText size={13} /> 로그 보기
        </Button>
        {installed && (
          <Button
            size="sm"
            variant="danger"
            className="ml-auto"
            disabled={busy}
            onClick={() => void run(() => uninstallModule(id), `${detail.name} 제거됨`)}
          >
            <Trash2 size={13} /> 제거
          </Button>
        )}
      </footer>
    </div>
  );
}

function DetailHeader({ name, onBack }: { name: string; onBack: () => void }) {
  return (
    <header className="flex items-center gap-2 border-b border-hwax-border px-3 py-2.5">
      <Button size="sm" variant="ghost" onClick={onBack} title="뒤로">
        <ArrowLeft size={16} />
      </Button>
      <h1 className="truncate text-sm font-semibold text-hwax-text">{name}</h1>
    </header>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-baseline gap-3">
      <span className="w-20 shrink-0 text-hwax-muted">{label}</span>
      <span className="flex items-center">{children}</span>
    </div>
  );
}
