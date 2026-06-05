import type { ReactNode } from 'react';
import type { ModuleState } from '../ipc/types';

type Tone = 'neutral' | 'accent' | 'warn' | 'error' | 'ok';

const TONE: Record<Tone, string> = {
  neutral: 'bg-hwax-elevated text-hwax-muted border-hwax-border',
  accent: 'bg-hwax-accent/15 text-hwax-accent border-hwax-accent/30',
  warn: 'bg-status-yellow/15 text-status-yellow border-status-yellow/30',
  error: 'bg-status-red/15 text-status-red border-status-red/30',
  ok: 'bg-status-green/15 text-status-green border-status-green/30',
};

interface BadgeProps {
  tone?: Tone;
  children: ReactNode;
  className?: string;
}

export function Badge({ tone = 'neutral', children, className = '' }: BadgeProps) {
  return (
    <span
      className={[
        'inline-flex items-center rounded-md border px-1.5 py-0.5',
        'text-[10px] font-medium leading-none tracking-wide',
        TONE[tone],
        className,
      ].join(' ')}
    >
      {children}
    </span>
  );
}

/** Korean labels + tone for each lifecycle state (v2 §6). */
const STATE_META: Record<ModuleState, { label: string; tone: Tone }> = {
  idle: { label: '대기', tone: 'neutral' },
  checking: { label: '확인 중', tone: 'warn' },
  installed: { label: '설치됨', tone: 'ok' },
  outdated: { label: '업데이트 가능', tone: 'warn' },
  not_installed: { label: '미설치', tone: 'neutral' },
  downloading: { label: '다운로드 중', tone: 'warn' },
  verifying: { label: '검증 중', tone: 'warn' },
  extracting: { label: '압축 해제 중', tone: 'warn' },
  swapping: { label: '교체 중', tone: 'warn' },
  running: { label: '실행 중', tone: 'accent' },
  stopped: { label: '중지됨', tone: 'neutral' },
  failed: { label: '실패', tone: 'error' },
  rolling_back: { label: '롤백 중', tone: 'error' },
  rolled_back: { label: '롤백됨', tone: 'warn' },
};

export function StateBadge({ state }: { state: ModuleState }) {
  const meta = STATE_META[state];
  return <Badge tone={meta.tone}>{meta.label}</Badge>;
}
