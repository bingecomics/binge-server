import type { ReactNode } from "react";
import { Button } from "./Button";
import { Dialog, DialogActions } from "./Dialog";
import { ErrorText } from "./text";

/** Asks before something that cannot be taken back. */
export function ConfirmDialog({
  open,
  onClose,
  title,
  children,
  detail,
  confirm,
  confirming,
  isPending,
  error,
  onConfirm,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  /** What will happen, in a sentence or two. */
  children: ReactNode;
  /** Something to show under those sentences, such as what it is done to. */
  detail?: ReactNode;
  /** The button's words, and its words while the thing is being done. */
  confirm: string;
  confirming: string;
  isPending: boolean;
  error: Error | null;
  onConfirm: () => void;
}) {
  return (
    <Dialog open={open} onClose={onClose} title={title}>
      <p className="mt-2 text-fg-muted text-metadata">{children}</p>
      {detail}
      {error ? <ErrorText className="mt-4">{error.message}</ErrorText> : null}
      <DialogActions onCancel={onClose}>
        <Button variant="danger" disabled={isPending} onClick={onConfirm}>
          {isPending ? confirming : confirm}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
