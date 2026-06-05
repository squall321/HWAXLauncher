import { useState } from 'react';
import {
  ArrowLeft,
  FilePlus2,
  Loader2,
  Lock,
  RefreshCcwDot,
  Trash,
} from 'lucide-react';
import { clearCache, makeDump } from '../ipc/commands';
import { useConfig } from '../hooks';
import { Button } from '../components/Button';
import { Toggle } from '../components/Toggle';

interface SettingsProps {
  onBack: () => void;
  /** Triggers the re-pairing flow (the only way to change the server address). */
  onRepair: () => void;
}

const LOG_LEVELS = ['trace', 'debug', 'info', 'warn'] as const;

/** Settings panel (v2 §4.4). Server address is read-only by design. */
export function Settings({ onBack, onRepair }: SettingsProps) {
  const { config, loading, saving, save, error } = useConfig();
  const [dumpPath, setDumpPath] = useState<string | null>(null);
  const [cacheCleared, setCacheCleared] = useState(false);
  const [working, setWorking] = useState<'dump' | 'cache' | null>(null);

  const onDump = async () => {
    setWorking('dump');
    try {
      setDumpPath(await makeDump());
    } finally {
      setWorking(null);
    }
  };

  const onClearCache = async () => {
    setWorking('cache');
    try {
      await clearCache();
      setCacheCleared(true);
      window.setTimeout(() => setCacheCleared(false), 2000);
    } finally {
      setWorking(null);
    }
  };

  return (
    <div className="flex h-full flex-col bg-hwax-bg">
      <header className="flex items-center gap-2 border-b border-hwax-border px-3 py-2.5">
        <Button size="sm" variant="ghost" onClick={onBack} title="뒤로">
          <ArrowLeft size={16} />
        </Button>
        <h1 className="text-sm font-semibold text-hwax-text">설정</h1>
      </header>

      <div className="flex-1 overflow-y-auto px-5 py-4">
        {loading || !config ? (
          <div className="flex items-center gap-2 py-10 text-sm text-hwax-muted">
            <Loader2 size={16} className="animate-spin" /> 설정을 불러오는 중…
          </div>
        ) : (
          <>
            {/* Identity — read-only */}
            <section className="space-y-2">
              <Field label="서버 주소">
                <div className="flex items-center gap-2">
                  <input
                    readOnly
                    value={config.server}
                    aria-label="서버 주소 (잠금)"
                    className="w-full cursor-not-allowed rounded-md border border-hwax-border bg-hwax-elevated px-3 py-1.5 font-mono text-sm text-hwax-muted focus:outline-none"
                  />
                  <span
                    className="inline-flex items-center gap-1 whitespace-nowrap text-[11px] text-hwax-muted"
                    title="서버 주소는 보안 정책상 잠겨 있습니다. 변경하려면 다시 페어링하세요."
                  >
                    <Lock size={12} /> 잠금
                  </span>
                </div>
              </Field>
              <Field label="Agent ID">
                <span data-selectable className="font-mono text-sm text-hwax-text">
                  {config.agent_id || '—'}
                </span>
              </Field>
              <Field label="채널">
                <span className="text-sm text-hwax-text">{config.channel}</span>
              </Field>
            </section>

            <Divider />

            {/* Behaviour toggles */}
            <section className="space-y-4">
              <Toggle
                id="auto-update"
                checked={config.auto_update}
                onChange={(v) => void save({ auto_update: v })}
                label="자동으로 업데이트 다운로드"
                disabled={saving}
              />
              <Toggle
                id="start-on-boot"
                checked={config.start_on_boot}
                onChange={(v) => void save({ start_on_boot: v })}
                label="Windows 시작 시 자동 실행"
                disabled={saving}
              />
              <Toggle
                id="telemetry"
                checked={config.telemetry_anonymous}
                onChange={(v) => void save({ telemetry_anonymous: v })}
                label="익명 사용 통계 전송"
                disabled={saving}
              />
            </section>

            <Divider />

            {/* Log level radios */}
            <section>
              <div className="mb-2 text-sm text-hwax-text">로그 레벨</div>
              <div className="flex flex-wrap gap-4">
                {LOG_LEVELS.map((level) => (
                  <label
                    key={level}
                    className="flex cursor-pointer items-center gap-1.5 text-sm text-hwax-muted"
                  >
                    <input
                      type="radio"
                      name="log_level"
                      value={level}
                      checked={config.log_level === level}
                      onChange={() => void save({ log_level: level })}
                      disabled={saving}
                      className="accent-[color:var(--hwax-accent)]"
                    />
                    <span className={config.log_level === level ? 'text-hwax-text' : ''}>
                      {level}
                    </span>
                  </label>
                ))}
              </div>
            </section>

            {error && (
              <p className="mt-3 rounded-md border border-status-red/30 bg-status-red/10 px-3 py-2 text-xs text-status-red">
                {error}
              </p>
            )}
          </>
        )}
      </div>

      {/* Maintenance actions */}
      <footer className="flex flex-wrap items-center gap-2 border-t border-hwax-border px-5 py-3">
        <Button size="sm" variant="secondary" disabled={working === 'dump'} onClick={() => void onDump()}>
          {working === 'dump' ? <Loader2 size={13} className="animate-spin" /> : <FilePlus2 size={13} />}
          진단 dump 만들기
        </Button>
        <Button size="sm" variant="secondary" onClick={onRepair}>
          <RefreshCcwDot size={13} /> 다시 페어링
        </Button>
        <Button size="sm" variant="ghost" disabled={working === 'cache'} onClick={() => void onClearCache()}>
          {working === 'cache' ? <Loader2 size={13} className="animate-spin" /> : <Trash size={13} />}
          {cacheCleared ? '캐시 비움' : '캐시 비우기'}
        </Button>
      </footer>

      {dumpPath && (
        <p
          data-selectable
          className="border-t border-hwax-border bg-hwax-elevated px-5 py-2 font-mono text-[11px] text-hwax-muted"
          title={dumpPath}
        >
          dump: {dumpPath}
        </p>
      )}
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-3">
      <span className="w-20 shrink-0 text-sm text-hwax-muted">{label}</span>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
}

function Divider() {
  return <hr className="my-5 border-hwax-border" />;
}
