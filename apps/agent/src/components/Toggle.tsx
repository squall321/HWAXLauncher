interface ToggleProps {
  checked: boolean;
  onChange: (next: boolean) => void;
  label?: string;
  description?: string;
  disabled?: boolean;
  id?: string;
}

/** Accessible switch (checkbox role) styled with the amber accent. */
export function Toggle({
  checked,
  onChange,
  label,
  description,
  disabled = false,
  id,
}: ToggleProps) {
  return (
    <label
      htmlFor={id}
      className={[
        'flex items-start gap-3 cursor-pointer select-none',
        disabled ? 'opacity-50 cursor-not-allowed' : '',
      ].join(' ')}
    >
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => !disabled && onChange(!checked)}
        className={[
          'relative mt-0.5 h-5 w-9 shrink-0 rounded-full transition-colors',
          checked ? 'bg-hwax-accent' : 'bg-hwax-border',
        ].join(' ')}
      >
        <span
          className={[
            'absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-hwax-bg transition-transform',
            checked ? 'translate-x-4' : 'translate-x-0',
          ].join(' ')}
        />
      </button>
      {(label || description) && (
        <span className="flex flex-col gap-0.5">
          {label && <span className="text-sm text-hwax-text">{label}</span>}
          {description && (
            <span className="text-xs text-hwax-muted">{description}</span>
          )}
        </span>
      )}
    </label>
  );
}
