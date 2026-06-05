interface ProgressBarProps {
  /** 0..=100 */
  percent: number;
  label?: string;
  /** Show the numeric percent at the right. */
  showPercent?: boolean;
  className?: string;
}

/** Determinate amber progress bar used by every install phase (v2 §10). */
export function ProgressBar({
  percent,
  label,
  showPercent = true,
  className = '',
}: ProgressBarProps) {
  const clamped = Math.max(0, Math.min(100, Math.round(percent)));
  return (
    <div className={['w-full', className].join(' ')}>
      {(label || showPercent) && (
        <div className="mb-1 flex items-center justify-between text-[11px] text-hwax-muted">
          {label && <span className="truncate">{label}</span>}
          {showPercent && <span className="tabular-nums">{clamped}%</span>}
        </div>
      )}
      <div
        className="h-1.5 w-full overflow-hidden rounded-full bg-hwax-border"
        role="progressbar"
        aria-valuenow={clamped}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div
          className="h-full rounded-full bg-hwax-accent transition-[width] duration-200 ease-out"
          style={{ width: `${clamped}%` }}
        />
      </div>
    </div>
  );
}
