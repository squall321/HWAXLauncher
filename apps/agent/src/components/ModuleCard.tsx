import {
  Download,
  FileText,
  Info,
  Play,
  RefreshCw,
  Square,
  XCircle,
} from 'lucide-react';
import type { ModuleView } from '../ipc/types';
import { Button } from './Button';
import { StateBadge } from './Badge';
import { ProgressBar } from './ProgressBar';
import { PHASE_LABEL, type InstallProgress } from '../hooks/useInstallProgress';

interface ModuleCardProps {
  module: ModuleView;
  /** Live install progress for this module, if an install is in flight. */
  progress?: InstallProgress | null;
  busy?: boolean;
  onInstall: (m: ModuleView) => void;
  onUpdate: (m: ModuleView) => void;
  onRun: (m: ModuleView) => void;
  onStop: (m: ModuleView) => void;
  onCancel: (m: ModuleView) => void;
  onDetail: (m: ModuleView) => void;
  onLogs: (m: ModuleView) => void;
}

const INSTALLING = new Set([
  'downloading',
  'verifying',
  'extracting',
  'swapping',
]);

/** Category glyph (the only place non-status marks are allowed, v2 §4.2). */
function categoryMark(category: string | null): string {
  switch (category) {
    case 'preprocessor':
      return '◆';
    case 'plugin':
      return '◈';
    case 'tool':
      return '◇';
    default:
      return '◆';
  }
}

export function ModuleCard({
  module: m,
  progress,
  busy = false,
  onInstall,
  onUpdate,
  onRun,
  onStop,
  onCancel,
  onDetail,
  onLogs,
}: ModuleCardProps) {
  const installing = INSTALLING.has(m.state) || (!!progress && m.state !== 'failed');
  const accent = m.color_accent ?? 'var(--hwax-accent)';

  // Sub-label: matches the wireframe ("설치됨 · 업데이트 가능 (2.2.0)" etc).
  let subline: string;
  switch (m.state) {
    case 'not_installed':
      subline = '미설치';
      break;
    case 'outdated':
      subline = `설치됨 · 업데이트 가능 (${m.latest_version ?? '?'})`;
      break;
    case 'failed':
      subline = '설치 실패';
      break;
    case 'running':
      subline = '실행 중';
      break;
    default:
      subline = '설치됨';
  }

  return (
    <div className="rounded-lg border border-hwax-border bg-hwax-elevated p-3">
      {/* Header row: mark + name + version */}
      <div className="flex items-start justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="text-base leading-none" style={{ color: accent }} aria-hidden>
            {categoryMark(m.category)}
          </span>
          <span className="truncate text-sm font-medium text-hwax-text" title={m.name}>
            {m.name}
          </span>
        </div>
        <span className="shrink-0 font-mono text-xs text-hwax-muted">
          {m.current_version ? `v${m.current_version}` : m.latest_version ? `v${m.latest_version}` : '—'}
        </span>
      </div>

      {/* Sub-line + state badge */}
      <div className="mt-1 flex items-center gap-2">
        <span className="truncate text-xs text-hwax-muted">{subline}</span>
        <StateBadge state={m.state} />
      </div>

      {/* Live install progress with the current phase label */}
      {installing && progress && (
        <div className="mt-2">
          <ProgressBar
            percent={progress.percent}
            label={PHASE_LABEL[progress.phase]}
          />
        </div>
      )}

      {/* Action row — mirrors the wireframe per state */}
      <div className="mt-3 flex flex-wrap items-center justify-end gap-1.5">
        {installing ? (
          <Button size="sm" variant="danger" onClick={() => onCancel(m)}>
            <XCircle size={13} /> 취소
          </Button>
        ) : (
          <>
            {m.state === 'outdated' && (
              <Button size="sm" variant="primary" disabled={busy} onClick={() => onUpdate(m)}>
                <RefreshCw size={13} /> 업데이트
              </Button>
            )}
            {m.state === 'not_installed' && (
              <Button size="sm" variant="primary" disabled={busy} onClick={() => onInstall(m)}>
                <Download size={13} /> 설치
              </Button>
            )}
            {m.state === 'running' ? (
              <Button size="sm" variant="secondary" onClick={() => onStop(m)}>
                <Square size={13} /> 중지
              </Button>
            ) : (
              (m.state === 'installed' ||
                m.state === 'outdated' ||
                m.state === 'stopped' ||
                m.state === 'rolled_back') && (
                <Button size="sm" variant="secondary" onClick={() => onRun(m)}>
                  <Play size={13} /> 실행
                </Button>
              )
            )}
            <Button size="sm" variant="ghost" onClick={() => onDetail(m)}>
              <Info size={13} /> 상세
            </Button>
            {m.state !== 'not_installed' && (
              <Button size="sm" variant="ghost" onClick={() => onLogs(m)}>
                <FileText size={13} /> 로그
              </Button>
            )}
          </>
        )}
      </div>
    </div>
  );
}
