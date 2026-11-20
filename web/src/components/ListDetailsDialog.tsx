import { useId, useState } from "react";
import type { ReadingList } from "../api/client";
import { useChangeList, useCreateList } from "../api/lists";
import { Button } from "./Button";
import { Dialog, DialogActions } from "./Dialog";
import { Field, fieldClass } from "./Field";
import { Form } from "./Form";

/** The longest summary the server keeps, in characters. */
const LONGEST_SUMMARY = 4000;

/** The two fields, and what saving them does: a new list, or the list's new details. */
function Details({
  list,
  onClose,
  onSaved,
}: {
  list?: ReadingList;
  onClose: () => void;
  onSaved?: (list: ReadingList) => void;
}) {
  const create = useCreateList();
  const change = useChangeList(list?.id ?? 0);
  const save = list ? change : create;
  const [name, setName] = useState(list?.name ?? "");
  const [summary, setSummary] = useState(list?.summary ?? "");
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
            {list ? "Save" : "Make the list"}
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
 * Asks for the name of a reading list and what it is about: of a new one,
 * or of `list` to change them. `onSaved` runs with the list as the server
 * has it.
 */
export function ListDetailsDialog({
  open,
  onClose,
  list,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  list?: ReadingList;
  onSaved?: (list: ReadingList) => void;
}) {
  return (
    <Dialog open={open} onClose={onClose} title={list ? "Name and summary" : "New reading list"}>
      <Details list={list} onClose={onClose} onSaved={onSaved} />
    </Dialog>
  );
}
