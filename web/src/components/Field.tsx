import { type InputHTMLAttributes, type ReactNode, useId } from "react";

/** How a text input looks. */
export const fieldClass =
  "mt-1 min-h-11 w-full rounded-control border border-outline bg-background px-3 text-fg " +
  "placeholder:text-fg-muted";

type Props = InputHTMLAttributes<HTMLInputElement> & {
  label: ReactNode;
  /** A line under the input that says what to type. */
  hint?: ReactNode;
  /** Classes for the whole field, such as the space above it. */
  className?: string;
};

/** A labelled text input, with an optional hint under it. */
export function Field({ label, hint, className = "", ...input }: Props) {
  const id = useId();
  const hintId = useId();
  return (
    <div className={className}>
      <label htmlFor={id} className="block font-semibold text-metadata">
        {label}
      </label>
      <input
        id={id}
        className={fieldClass}
        aria-describedby={hint ? hintId : undefined}
        {...input}
      />
      {hint ? (
        <p id={hintId} className="mt-1 text-caption text-fg-muted">
          {hint}
        </p>
      ) : null}
    </div>
  );
}

/** A box to tick, with a line under its name that says what ticking it does. */
export function Tick({
  name,
  under,
  checked,
  disabled,
  onChange,
  className = "mt-3",
}: {
  name: string;
  under: ReactNode;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
  /** Classes for the whole of it, such as the space above it. */
  className?: string;
}) {
  return (
    <label className={`flex items-start gap-3 ${className}`}>
      <input
        type="checkbox"
        className="mt-0.5 size-5 shrink-0 accent-accent"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="min-w-0">
        <span className="block font-semibold text-metadata">{name}</span>
        <span className="block text-caption text-fg-muted">{under}</span>
      </span>
    </label>
  );
}
