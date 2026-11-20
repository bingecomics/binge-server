import type { ReactNode } from "react";

/** A list of what is known about something, each fact a `Fact` with its label. */
export function Facts({ className = "", children }: { className?: string; children: ReactNode }) {
  return (
    <dl
      className={`grid max-w-prose grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-2 text-metadata ${className}`}
    >
      {children}
    </dl>
  );
}

export function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="text-fg-muted">{label}</dt>
      <dd className="break-words">{children}</dd>
    </>
  );
}
