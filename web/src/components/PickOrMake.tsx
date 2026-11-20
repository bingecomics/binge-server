import { useState } from "react";
import { Button } from "./Button";
import { DialogActions } from "./Dialog";
import { Field } from "./Field";
import { Form } from "./Form";
import { choiceClass } from "./Menu";

/**
 * The things of the reader's to choose one from, and the way to make a new
 * one by its name: where something is added to a reading list or to a
 * collection. `kind` is what such a thing is called, and `holds` what one
 * of them says it holds.
 */
export function PickOrMake<T extends { id: number; name: string }>({
  kind,
  choices,
  holds,
  busy,
  problem,
  onPick,
  onMake,
  onClose,
}: {
  kind: string;
  choices: T[];
  holds: (choice: T) => string;
  busy: boolean;
  problem?: string;
  onPick: (choice: T) => void;
  onMake: (name: string) => void;
  onClose: () => void;
}) {
  const [name, setName] = useState("");
  return (
    <Form
      className="mt-4"
      problem={problem}
      onSubmit={() => onMake(name)}
      actions={
        <DialogActions onCancel={onClose}>
          <Button type="submit" variant="primary" disabled={busy || name.trim() === ""}>
            Make the {kind}
          </Button>
        </DialogActions>
      }
    >
      {choices.length > 0 ? (
        <ul className="-mx-3 mb-4 max-h-64 overflow-y-auto">
          {choices.map((choice) => (
            <li key={choice.id}>
              <button
                type="button"
                disabled={busy}
                onClick={() => onPick(choice)}
                className={`${choiceClass} justify-between`}
              >
                <span className="min-w-0 truncate font-semibold">{choice.name}</span>
                <span className="shrink-0 text-caption text-fg-muted">{holds(choice)}</span>
              </button>
            </li>
          ))}
        </ul>
      ) : null}
      <Field
        label={choices.length > 0 ? `Or a new ${kind}` : `A new ${kind}`}
        value={name}
        onChange={(event) => setName(event.target.value)}
        placeholder="Its name"
        maxLength={200}
      />
    </Form>
  );
}
