import { useState } from "react";
import { useAddUser, useChangeUser, useRemoveUser, useUsers } from "../api/account";
import type { Library, User } from "../api/client";
import { useLibraries } from "../api/libraries";
import { Button } from "../components/Button";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { Dialog, DialogActions } from "../components/Dialog";
import { Field } from "../components/Field";
import { Form } from "../components/Form";
import { ListRow, Section } from "../components/Section";
import { ErrorText } from "../components/text";
import { NewPasswordField, PASSWORD_HINT } from "./NewPasswordField";

/** The libraries a member may read, as boxes to tick. */
function LibraryBoxes({
  libraries,
  chosen,
  onChange,
}: {
  libraries: Library[];
  chosen: number[];
  onChange: (chosen: number[]) => void;
}) {
  return (
    <fieldset className="mt-4">
      <legend className="font-semibold text-metadata">May read</legend>
      {libraries.length === 0 ? (
        <p className="mt-1 text-fg-muted text-metadata">
          There are no libraries yet. You can choose them later.
        </p>
      ) : (
        <ul className="mt-1">
          {libraries.map((library) => (
            <li key={library.id}>
              <label className="flex min-h-11 items-center gap-3">
                <input
                  type="checkbox"
                  className="size-5 shrink-0 accent-accent"
                  checked={chosen.includes(library.id)}
                  onChange={(event) =>
                    onChange(
                      event.target.checked
                        ? [...chosen, library.id]
                        : chosen.filter((id) => id !== library.id),
                    )
                  }
                />
                <span className="min-w-0 truncate">{library.name}</span>
              </label>
            </li>
          ))}
        </ul>
      )}
    </fieldset>
  );
}

function AddUserForm({ libraries, onClose }: { libraries: Library[]; onClose: () => void }) {
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [chosen, setChosen] = useState<number[]>([]);
  const add = useAddUser();

  return (
    <Form
      onSubmit={() => add.mutate({ name, password, libraries: chosen }, { onSuccess: onClose })}
      problem={add.error?.message}
      className="mt-2"
      actions={
        <DialogActions onCancel={onClose}>
          <Button variant="primary" type="submit" disabled={add.isPending}>
            {add.isPending ? "Adding…" : "Add user"}
          </Button>
        </DialogActions>
      }
    >
      <p className="text-fg-muted text-metadata">
        They sign in with this name and password, and keep their own place in every book.
      </p>
      <Field
        label="Name"
        className="mt-5"
        value={name}
        onChange={(event) => setName(event.target.value)}
        autoComplete="off"
        autoCapitalize="off"
        spellCheck={false}
        required
      />
      <NewPasswordField
        className="mt-4"
        value={password}
        onChange={(event) => setPassword(event.target.value)}
        hint={`${PASSWORD_HINT} Tell it to them; they can change it once signed in.`}
        required
      />
      <LibraryBoxes libraries={libraries} chosen={chosen} onChange={setChosen} />
    </Form>
  );
}

function EditUserForm({
  user,
  libraries,
  onClose,
}: {
  user: User;
  libraries: Library[];
  onClose: () => void;
}) {
  const [password, setPassword] = useState("");
  const [chosen, setChosen] = useState(user.libraries);
  const change = useChangeUser();

  function submit() {
    change.mutate(
      { id: user.id, change: { libraries: chosen, password: password || null } },
      { onSuccess: onClose },
    );
  }

  return (
    <Form
      onSubmit={submit}
      problem={change.error?.message}
      className="mt-2"
      actions={
        <DialogActions onCancel={onClose}>
          <Button variant="primary" type="submit" disabled={change.isPending}>
            {change.isPending ? "Saving…" : "Save"}
          </Button>
        </DialogActions>
      }
    >
      <LibraryBoxes libraries={libraries} chosen={chosen} onChange={setChosen} />
      <NewPasswordField
        label={
          <>
            New password <span className="font-normal text-fg-muted">(optional)</span>
          </>
        }
        className="mt-4"
        value={password}
        onChange={(event) => setPassword(event.target.value)}
        hint="Leave it empty to keep the one they have. A new one signs them out of their browsers."
      />
    </Form>
  );
}

/** What a user may read, in words. */
function reads(user: User, libraries: Library[]): string {
  if (user.role === "owner") {
    return "Owner · reads every library";
  }
  const names = libraries
    .filter((library) => user.libraries.includes(library.id))
    .map((library) => library.name);
  return names.length ? `Reads ${names.join(", ")}` : "No libraries yet";
}

/** Everyone who may use the server, for the owner to manage. */
export function UsersSection() {
  const users = useUsers();
  const libraries = useLibraries().data ?? [];
  const remove = useRemoveUser();
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<User | null>(null);
  const [removing, setRemoving] = useState<User | null>(null);

  function closeRemove() {
    setRemoving(null);
    remove.reset();
  }

  return (
    <Section title="Users" action={<Button onClick={() => setAdding(true)}>Add a user</Button>}>
      {users.isError ? <ErrorText>{users.error.message}</ErrorText> : null}
      <ul>
        {users.data?.map((user) => (
          <ListRow key={user.id} title={user.name} detail={reads(user, libraries)}>
            {user.role === "owner" ? null : (
              <>
                <Button onClick={() => setEditing(user)}>Edit</Button>
                <Button variant="quiet" onClick={() => setRemoving(user)}>
                  Remove
                </Button>
              </>
            )}
          </ListRow>
        ))}
      </ul>

      <Dialog open={adding} onClose={() => setAdding(false)} title="Add a user">
        <AddUserForm libraries={libraries} onClose={() => setAdding(false)} />
      </Dialog>
      <Dialog
        open={editing !== null}
        onClose={() => setEditing(null)}
        title={editing ? `Edit ${editing.name}` : ""}
      >
        {editing ? (
          <EditUserForm user={editing} libraries={libraries} onClose={() => setEditing(null)} />
        ) : null}
      </Dialog>
      <ConfirmDialog
        open={removing !== null}
        onClose={closeRemove}
        title={removing ? `Remove ${removing.name}?` : ""}
        confirm="Remove user"
        confirming="Removing…"
        isPending={remove.isPending}
        error={remove.error}
        onConfirm={() => removing && remove.mutate(removing.id, { onSuccess: closeRemove })}
      >
        They can no longer sign in, their apps are disconnected, and their place in every book is
        forgotten.
      </ConfirmDialog>
    </Section>
  );
}
