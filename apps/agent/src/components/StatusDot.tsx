import type { ModuleState, StatusColor } from '../ipc/types';

/** Map a tray status color → Tailwind background. */
const COLOR: Record<StatusColor, string> = {
  green: 'bg-status-green',
  yellow: 'bg-status-yellow',
  red: 'bg-status-red',
};

/**
 * Reduce a module lifecycle state to a tray-dot color.
 * Active transitions are "warn" (yellow), terminal failures "error" (red),
 * everything healthy is green.
 */
export function stateToColor(state: ModuleState): StatusColor {
  switch (state) {
    case 'failed':
    case 'rolling_back':
      return 'red';
    case 'checking':
    case 'downloading':
    case 'verifying':
    case 'extracting':
    case 'swapping':
    case 'outdated':
    case 'rolled_back':
      return 'yellow';
    default:
      // idle, installed, not_installed, running, stopped
      return 'green';
  }
}

interface StatusDotProps {
  color: StatusColor;
  /** Pulse for in-progress states. */
  pulse?: boolean;
  className?: string;
  title?: string;
}

export function StatusDot({ color, pulse = false, className = '', title }: StatusDotProps) {
  return (
    <span
      title={title}
      className={[
        'relative inline-block h-2.5 w-2.5 rounded-full',
        COLOR[color],
        className,
      ].join(' ')}
    >
      {pulse && (
        <span
          className={[
            'absolute inset-0 rounded-full opacity-60 animate-ping',
            COLOR[color],
          ].join(' ')}
        />
      )}
    </span>
  );
}
