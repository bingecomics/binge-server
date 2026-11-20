import type { ReactNode } from "react";
import { Brand } from "../components/Brand";

/** The page someone sees before they are in: a card in the middle of the window. */
export function Entrance({
  title,
  lead,
  children,
}: {
  title: string;
  lead?: ReactNode;
  children: ReactNode;
}) {
  return (
    <main className="grid min-h-dvh place-items-center px-4 py-8">
      <div className="w-full max-w-md">
        <p className="mb-6 text-center text-wordmark">
          <Brand />
        </p>
        <section className="rounded-card border border-divider bg-surface p-6 md:p-8">
          <h1 className="font-semibold text-section-title">{title}</h1>
          {lead ? <p className="mt-2 text-fg-muted text-metadata">{lead}</p> : null}
          {children}
        </section>
      </div>
    </main>
  );
}
