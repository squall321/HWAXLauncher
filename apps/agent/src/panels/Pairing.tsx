import { useEffect, useState } from 'react';
import { Copy, ExternalLink, Link2, Loader2 } from 'lucide-react';
import { completePairing, startPairing } from '../ipc/commands';
import type { AgentStatus, PairingInfo } from '../ipc/types';
import { Button } from '../components/Button';

interface PairingProps {
  /** Called with the fresh status once enrollment succeeds. */
  onPaired: (status: AgentStatus) => void;
}

/**
 * Pairing / enrollment panel (v2 §4 + openapi `/enroll`).
 *
 * Flow:
 *  1. `start_pairing()` → the agent shows the operator URL + a short code.
 *  2. The operator issues a single-use `enrollment_token` in the HEAXHub admin
 *     UI; the user pastes it here.
 *  3. `complete_pairing({ enrollment_token })` exchanges it for the device JWT
 *     pair (stored by Rust in Credential Manager — never in this UI).
 */
export function Pairing({ onPaired }: PairingProps) {
  const [info, setInfo] = useState<PairingInfo | null>(null);
  const [token, setToken] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let active = true;
    startPairing()
      .then((i) => active && setInfo(i))
      .catch((e) => active && setError(String(e)));
    return () => {
      active = false;
    };
  }, []);

  const submit = async () => {
    const trimmed = token.trim();
    if (!trimmed) return;
    setSubmitting(true);
    setError(null);
    try {
      const status = await completePairing(trimmed);
      onPaired(status);
    } catch (e) {
      setError(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  const copyUrl = async () => {
    if (!info) return;
    try {
      await navigator.clipboard.writeText(info.url);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      /* clipboard may be unavailable in the webview; ignore */
    }
  };

  return (
    <div className="flex h-full flex-col bg-hwax-bg p-6">
      <header className="mb-5">
        <h1 className="text-lg font-semibold text-hwax-text">기기 페어링</h1>
        <p className="mt-1 text-sm text-hwax-muted">
          HEAXHub 관리자에게서 받은 등록 토큰으로 이 기기를 연결합니다.
        </p>
      </header>

      {/* Step 1 — operator URL + code */}
      <section className="rounded-lg border border-hwax-border bg-hwax-elevated p-4">
        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-hwax-muted">
          <Link2 size={14} /> 1. 관리자 콘솔에서 열기
        </div>
        {info ? (
          <div className="mt-3 space-y-3">
            <div className="flex items-center gap-2">
              <code
                data-selectable
                className="flex-1 truncate rounded-md bg-hwax-bg px-2.5 py-2 font-mono text-xs text-hwax-text"
                title={info.url}
              >
                {info.url}
              </code>
              <Button size="sm" variant="ghost" onClick={copyUrl} title="URL 복사">
                <Copy size={14} /> {copied ? '복사됨' : '복사'}
              </Button>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-xs text-hwax-muted">페어링 코드</span>
              <span
                data-selectable
                className="rounded-md bg-hwax-bg px-3 py-1 font-mono text-base tracking-[0.3em] text-hwax-accent"
              >
                {info.code}
              </span>
            </div>
          </div>
        ) : (
          <div className="mt-3 flex items-center gap-2 text-sm text-hwax-muted">
            <Loader2 size={16} className="animate-spin" /> 페어링 정보를 불러오는 중…
          </div>
        )}
      </section>

      {/* Step 2 — paste enrollment token */}
      <section className="mt-4 rounded-lg border border-hwax-border bg-hwax-elevated p-4">
        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-hwax-muted">
          <ExternalLink size={14} /> 2. 등록 토큰 입력
        </div>
        <label htmlFor="enrollment-token" className="sr-only">
          등록 토큰
        </label>
        <input
          id="enrollment-token"
          type="text"
          autoComplete="off"
          spellCheck={false}
          value={token}
          onChange={(e) => setToken(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && void submit()}
          placeholder="enrollment_token 붙여넣기"
          className="mt-3 w-full rounded-md border border-hwax-border bg-hwax-bg px-3 py-2 font-mono text-sm text-hwax-text placeholder:text-hwax-muted/60 focus:border-hwax-accent focus:outline-none"
        />
      </section>

      {error && (
        <p className="mt-3 rounded-md border border-status-red/30 bg-status-red/10 px-3 py-2 text-xs text-status-red">
          {error}
        </p>
      )}

      <div className="mt-auto pt-5">
        <Button
          variant="primary"
          className="w-full"
          disabled={submitting || token.trim().length === 0}
          onClick={() => void submit()}
        >
          {submitting && <Loader2 size={15} className="animate-spin" />}
          페어링 완료
        </Button>
      </div>
    </div>
  );
}
