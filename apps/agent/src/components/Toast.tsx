import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { CheckCircle2, Info, X, XCircle } from 'lucide-react';

/**
 * Minimal in-app toast surface. The agent's panel is a small tray window, so
 * toasts stack at the bottom, auto-dismiss, and never block. Use it for IPC
 * outcomes (install done / failed, rollback, errors) that would otherwise be
 * invisible to the user.
 */
export type ToastKind = 'info' | 'success' | 'error';

interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
}

interface ToastApi {
  push: (kind: ToastKind, message: string) => void;
}

const ToastCtx = createContext<ToastApi | null>(null);

export function useToast(): ToastApi {
  const ctx = useContext(ToastCtx);
  if (!ctx) throw new Error('useToast must be used within <ToastProvider>');
  return ctx;
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const seq = useRef(0);

  const remove = useCallback((id: number) => {
    setToasts((ts) => ts.filter((t) => t.id !== id));
  }, []);

  const push = useCallback(
    (kind: ToastKind, message: string) => {
      seq.current += 1;
      const id = seq.current;
      // Keep at most 4 visible; newest at the bottom.
      setToasts((ts) => [...ts.slice(-3), { id, kind, message }]);
      window.setTimeout(() => remove(id), kind === 'error' ? 6000 : 4000);
    },
    [remove],
  );

  const api = useMemo<ToastApi>(() => ({ push }), [push]);

  return (
    <ToastCtx.Provider value={api}>
      {children}
      <div className="pointer-events-none fixed inset-x-0 bottom-0 z-50 flex flex-col items-center gap-1.5 p-3">
        {toasts.map((t) => (
          <ToastItem key={t.id} toast={t} onClose={() => remove(t.id)} />
        ))}
      </div>
    </ToastCtx.Provider>
  );
}

function ToastItem({ toast, onClose }: { toast: Toast; onClose: () => void }) {
  const { kind, message } = toast;
  const Icon = kind === 'success' ? CheckCircle2 : kind === 'error' ? XCircle : Info;
  const tone =
    kind === 'success'
      ? 'border-status-green/40 text-status-green'
      : kind === 'error'
        ? 'border-status-red/40 text-status-red'
        : 'border-hwax-border text-hwax-text';
  return (
    <div
      role="status"
      className={`pointer-events-auto flex w-full max-w-[420px] items-start gap-2 rounded-md border ${tone} bg-hwax-elevated px-3 py-2 shadow-lg`}
    >
      <Icon size={15} className="mt-0.5 shrink-0" />
      <span className="min-w-0 flex-1 break-words text-xs text-hwax-text">{message}</span>
      <button
        onClick={onClose}
        className="shrink-0 text-hwax-muted hover:text-hwax-text"
        aria-label="닫기"
      >
        <X size={13} />
      </button>
    </div>
  );
}
