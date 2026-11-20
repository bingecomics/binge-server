import type { Library } from "../api/client";
import { useRemoveLibrary } from "../api/libraries";
import { ConfirmDialog } from "./ConfirmDialog";
import { PathText } from "./text";

export function RemoveLibraryDialog({
  open,
  library,
  onClose,
  onRemoved,
}: {
  open: boolean;
  library: Library;
  onClose: () => void;
  /** Runs once the library is removed. May return a promise to be waited for. */
  onRemoved: () => unknown;
}) {
  const remove = useRemoveLibrary(onRemoved);

  function close() {
    onClose();
    remove.reset();
  }

  return (
    <ConfirmDialog
      open={open}
      onClose={close}
      title={`Remove “${library.name}”?`}
      detail={library.path ? <PathText path={library.path} className="mt-3" /> : null}
      confirm="Remove library"
      confirming="Removing…"
      isPending={remove.isPending}
      error={remove.error}
      onConfirm={() => remove.mutate(library.id)}
    >
      The server forgets this library. The folder and the comics in it stay exactly as they are.
      Covers and what was read are kept, in case you add it again.
    </ConfirmDialog>
  );
}
