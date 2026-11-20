import { type ReactNode, useId } from "react";
import { ErrorText } from "./text";

/**
 * A form that says what went wrong under its fields, and tells screen
 * readers that the sentence is about the form.
 */
export function Form({
  onSubmit,
  problem,
  actions,
  className,
  children,
}: {
  onSubmit: () => void;
  /** What went wrong, as a sentence. Nothing when nothing did. */
  problem?: string | null;
  /** The buttons that end the form. */
  actions: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  const problemId = useId();
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
      className={className}
      aria-describedby={problem ? problemId : undefined}
    >
      {children}
      {problem ? (
        <ErrorText id={problemId} className="mt-4">
          {problem}
        </ErrorText>
      ) : null}
      {actions}
    </form>
  );
}
