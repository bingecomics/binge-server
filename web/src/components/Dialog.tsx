import { type ReactNode, useEffect, useId, useRef } from "react";
import { Button } from "./Button";

/** A modal dialog. Escape and `onClose` close it; focus returns to where it was. */
export function Dialog({
  open,
  onClose,
  title,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) {
      return;
    }
    if (open && !dialog.open) {
      dialog.showModal();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  }, [open]);

  return (
    <dialog
      ref={ref}
      onClose={onClose}
      aria-labelledby={titleId}
      className="m-auto w-[min(32rem,calc(100vw-2rem))] rounded-card border border-divider bg-surface p-6 text-fg shadow-xl backdrop:bg-black/60"
    >
      <h2 id={titleId} className="font-semibold text-section-title">
        {title}
      </h2>
      {/* Nothing inside is mounted while closed, so each opening starts fresh. */}
      {open ? children : null}
    </dialog>
  );
}

/** The row of buttons that ends a dialog: Cancel, then what the dialog is for. */
export function DialogActions({
  onCancel,
  children,
}: {
  onCancel: () => void;
  children: ReactNode;
}) {
  return (
    <div className="mt-6 flex justify-end gap-2">
      <Button variant="quiet" onClick={onCancel}>
        Cancel
      </Button>
      {children}
    </div>
  );
}
