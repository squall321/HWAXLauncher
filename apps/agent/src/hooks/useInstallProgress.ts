import { useEffect, useState } from 'react';
import { onInstallProgress } from '../ipc/events';
import type { InstallPhase, InstallProgressEvent } from '../ipc/types';

export interface InstallProgress {
  phase: InstallPhase;
  /** 0..=100 */
  percent: number;
}

/** Ordered phases as they appear in the install state machine (v2 §6/§10). */
export const INSTALL_PHASES: InstallPhase[] = [
  'download',
  'verify',
  'extract',
  'check',
  'swap',
];

/** Human labels for the progress UI. */
export const PHASE_LABEL: Record<InstallPhase, string> = {
  download: '다운로드',
  verify: '검증 (sha256)',
  extract: '압축 해제',
  check: '설치 후 검사',
  swap: '교체 (원자적)',
};

/**
 * Subscribes to `install:progress` and exposes the latest phase/percent.
 *
 * - Pass a specific `id` to track one module (the others are ignored).
 * - Pass `null`/omit to track *any* in-flight install (last writer wins) — used
 *   by the tray list to show a single active bar.
 *
 * The map keeps the most recent progress per module id so multiple cards can
 * render their own bar from one shared listener.
 */
export function useInstallProgress(id?: string | null) {
  const [byId, setById] = useState<Record<string, InstallProgress>>({});

  useEffect(() => {
    let active = true;
    const p = onInstallProgress((evt: InstallProgressEvent) => {
      if (!active) return;
      setById((prev) => ({
        ...prev,
        [evt.id]: { phase: evt.phase, percent: evt.percent },
      }));
    });
    return () => {
      active = false;
      void p.then((un) => un());
    };
  }, []);

  const progress = id ? (byId[id] ?? null) : null;
  return { progress, byId };
}
