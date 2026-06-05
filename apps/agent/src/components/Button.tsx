import type { ButtonHTMLAttributes, ReactNode } from 'react';

type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';
type Size = 'sm' | 'md';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  children: ReactNode;
}

const VARIANT: Record<Variant, string> = {
  // Amber accent CTA (install / update / pair).
  primary:
    'bg-hwax-accent text-hwax-bg hover:bg-hwax-accent-hover font-medium',
  // Neutral elevated button (run / detail).
  secondary:
    'bg-hwax-elevated text-hwax-text border border-hwax-border hover:border-hwax-accent',
  // Borderless (logs / open folder).
  ghost: 'bg-transparent text-hwax-muted hover:text-hwax-text hover:bg-hwax-elevated',
  // Destructive (uninstall / rollback).
  danger:
    'bg-transparent text-status-red border border-status-red/40 hover:bg-status-red/10',
};

const SIZE: Record<Size, string> = {
  sm: 'h-7 px-2.5 text-xs rounded-md gap-1',
  md: 'h-9 px-3.5 text-sm rounded-md gap-1.5',
};

/** Minimal Tailwind button — no heavy UI lib (workstream constraint). */
export function Button({
  variant = 'secondary',
  size = 'md',
  className = '',
  children,
  disabled,
  ...rest
}: ButtonProps) {
  return (
    <button
      className={[
        'inline-flex items-center justify-center whitespace-nowrap',
        'transition-colors select-none',
        'disabled:opacity-40 disabled:cursor-not-allowed',
        SIZE[size],
        VARIANT[variant],
        className,
      ].join(' ')}
      disabled={disabled}
      {...rest}
    >
      {children}
    </button>
  );
}
