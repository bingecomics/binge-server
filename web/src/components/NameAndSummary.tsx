import { useId, useState } from "react";
import { Button } from "./Button";
import { Dialog, DialogActions } from "./Dialog";
import { Field, fieldClass } from "./Field";
import { Form } from "./Form";

/** The longest summary the server keeps, in characters. */
const LONGEST_SUMMARY = 4000;

/** What makes a thing with a name and a summary, or gives one new ones. */
type Save<T> = {
  mutate: (
    details: { name: string; summary: string },
    options: { onSuccess: (saved: T) => void },
  ) => void;
  isPending: boolean;
  error: Error | null;
  /** Forgets what went wrong the last time. */
  reset: () => void;
};

type Props<T> = {
  /** What has them now. A new thing has none. */
  of?: { name: string; summary?: string | null };
  save: Save<T>;
  saveLabel: string;
  onClose: () => void;
  onSaved?: (saved: T) => void;
};

/** The two fields, filled with what `of` has when the dialog opens. */
function Details<T>({ of, save, saveLabel, onClose, onSaved }: Props<T>) {
  const [name, setName] = useState(of?.name ?? "");
  const [summary, setSummary] = useState(of?.summary ?? "");
  const summaryId = useId();

  return (
    <Form
      className="mt-4"
      problem={save.error?.message}
      onSubmit={() =>
        save.mutate(
          { name, summary },
          {
            onSuccess: (saved) => {
              onSaved?.(saved);
              onClose();
            },
          },
        )
      }
      actions={
        <DialogActions onCancel={onClose}>
          <Button type="submit" variant="primary" disabled={save.isPending || name.trim() === ""}>
            {saveLabel}
          </Button>
        </DialogActions>
      }
    >
      <Field
        label="Name"
        value={name}
        onChange={(event) => setName(event.target.value)}
        maxLength={200}
        autoFocus
      />
      <label htmlFor={summaryId} className="mt-4 block font-semibold text-metadata">
        What it is about
      </label>
      <textarea
        id={summaryId}
        value={summary}
        onChange={(event) => setSummary(event.target.value)}
        maxLength={LONGEST_SUMMARY}
        rows={4}
        className={`${fieldClass} py-2`}
      />
    </Form>
  );
}

/**
 * Asks for the name of a reading list or a collection and what it is about:
 * of a new one, or of `of` to change them. `save` is what makes it or
 * changes it, and `onSaved` runs with what was saved as the server has it.
 */
export function NameAndSummaryDialog<T>({
  open,
  title,
  onClose,
  ...props
}: Props<T> & { open: boolean; title: string }) {
  // What went wrong is not there again when the dialog is opened again.
  function close() {
    props.save.reset();
    onClose();
  }
  return (
    <Dialog open={open} onClose={close} title={title}>
      <Details {...props} onClose={close} />
    </Dialog>
  );
}
