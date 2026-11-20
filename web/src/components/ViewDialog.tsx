import { useState } from "react";
import type { View } from "../api/client";
import { keptOf, useAddView, useChangeView } from "../api/views";
import type { Asked } from "../narrowing";
import { Button } from "./Button";
import { Dialog, DialogActions } from "./Dialog";
import { Field, Tick } from "./Field";
import { Form } from "./Form";

function Details({
  view,
  library,
  onClose,
}: {
  view?: View;
  library?: { id: number; asked: Asked };
  onClose: () => void;
}) {
  const add = useAddView();
  const change = useChangeView();
  const save = view ? change : add;
  const [name, setName] = useState(view?.name ?? "");
  const [onHome, setOnHome] = useState(view?.on_home ?? true);
  const [inMenu, setInMenu] = useState(view?.in_menu ?? false);

  function submit() {
    const kept = { name, on_home: onHome, in_menu: inMenu };
    if (view) {
      change.mutate({ id: view.id, change: kept }, { onSuccess: onClose });
    } else if (library) {
      add.mutate(
        { ...kept, library_id: library.id, asked: keptOf(library.asked) },
        { onSuccess: onClose },
      );
    }
  }

  return (
    <Form
      className="mt-4"
      problem={save.error?.message}
      onSubmit={submit}
      actions={
        <DialogActions onCancel={onClose}>
          <Button type="submit" variant="primary" disabled={save.isPending || name.trim() === ""}>
            {view ? "Save" : "Keep the view"}
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
        hint={
          view
            ? undefined
            : "The order and what the library is narrowed by now are kept under this name, for everyone who reads the library."
        }
      />
      <Tick
        name="A row on Home"
        under="Its first series show on Home, with the way to the rest."
        checked={onHome}
        onChange={setOnHome}
      />
      <Tick
        name="In the menu"
        under="It is listed beside the page, with the libraries."
        checked={inMenu}
        onChange={setInMenu}
      />
    </Form>
  );
}

/**
 * Asks for the name of a view and where it shows: of a new one that keeps
 * what a `library`'s page asks now, or of `view` to change them.
 */
export function ViewDialog({
  open,
  onClose,
  view,
  library,
}: {
  open: boolean;
  onClose: () => void;
  view?: View;
  library?: { id: number; asked: Asked };
}) {
  return (
    <Dialog open={open} onClose={onClose} title={view ? "Name and place" : "Keep this view"}>
      <Details view={view} library={library} onClose={onClose} />
    </Dialog>
  );
}
