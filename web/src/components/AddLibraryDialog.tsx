import { useNavigate } from "@tanstack/react-router";
import { type FormEvent, useId, useState } from "react";
import { useAddLibrary } from "../api/libraries";
import { Button } from "./Button";
import { Dialog, DialogActions } from "./Dialog";
import { fieldClass } from "./Field";
import { FromFilesTicks, MAKES_BOTH } from "./FromFiles";
import { ErrorText } from "./text";

function AddLibraryForm({ onClose }: { onClose: () => void }) {
  const [path, setPath] = useState("");
  const [name, setName] = useState("");
  const [fromFiles, setFromFiles] = useState(MAKES_BOTH);
  const pathId = useId();
  const nameId = useId();
  const errorId = useId();
  const add = useAddLibrary();
  const navigate = useNavigate();

  function submit(event: FormEvent) {
    event.preventDefault();
    add.mutate(
      { path, name: name.trim() || null, from_files: fromFiles },
      {
        onSuccess: (library) => {
          onClose();
          void navigate({
            to: "/libraries/$libraryId",
            params: { libraryId: library.id },
          });
        },
      },
    );
  }

  return (
    <form onSubmit={submit} className="mt-2">
      <p className="text-fg-muted text-metadata">
        Point the server at a folder of comics. It reads the folder and never changes anything in
        it.
      </p>

      <label htmlFor={pathId} className="mt-5 block font-semibold text-metadata">
        Folder on the server
      </label>
      <input
        id={pathId}
        className={`${fieldClass} font-mono text-metadata`}
        value={path}
        onChange={(event) => setPath(event.target.value)}
        placeholder="/Volumes/Media/Comics"
        autoComplete="off"
        autoCapitalize="off"
        spellCheck={false}
        required
        aria-describedby={add.isError ? errorId : undefined}
        aria-invalid={add.isError}
      />
      <p className="mt-1 text-caption text-fg-muted">
        The full path, as the computer running the server sees it. Comics are put into series by
        their names, wherever they are inside it.
      </p>

      <label htmlFor={nameId} className="mt-4 block font-semibold text-metadata">
        Name <span className="font-normal text-fg-muted">(optional)</span>
      </label>
      <input
        id={nameId}
        className={fieldClass}
        value={name}
        onChange={(event) => setName(event.target.value)}
        placeholder="The folder's name"
        autoComplete="off"
      />

      <div className="mt-5">
        <FromFilesTicks from={fromFiles} onChange={setFromFiles} heading />
      </div>

      {add.isError ? (
        <ErrorText id={errorId} className="mt-4">
          {add.error.message}
        </ErrorText>
      ) : null}

      <DialogActions onCancel={onClose}>
        <Button variant="primary" type="submit" disabled={add.isPending || path.trim() === ""}>
          {add.isPending ? "Adding…" : "Add library"}
        </Button>
      </DialogActions>
    </form>
  );
}

export function AddLibraryDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Dialog open={open} onClose={onClose} title="Add a library">
      <AddLibraryForm onClose={onClose} />
    </Dialog>
  );
}
