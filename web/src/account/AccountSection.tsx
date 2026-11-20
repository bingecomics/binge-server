import { useState } from "react";
import { useChangePassword, useMe, useSignOut } from "../api/account";
import { Button } from "../components/Button";
import { Dialog, DialogActions } from "../components/Dialog";
import { Field } from "../components/Field";
import { Form } from "../components/Form";
import { Row, Section } from "../components/Section";
import { ErrorText } from "../components/text";
import { NewPasswordField } from "./NewPasswordField";

function PasswordForm({ onClose, onChanged }: { onClose: () => void; onChanged: () => void }) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const change = useChangePassword();

  return (
    <Form
      onSubmit={() => change.mutate({ current, new: next }, { onSuccess: onChanged })}
      problem={change.error?.message}
      className="mt-2"
      actions={
        <DialogActions onCancel={onClose}>
          <Button variant="primary" type="submit" disabled={change.isPending}>
            {change.isPending ? "Changing…" : "Change password"}
          </Button>
        </DialogActions>
      }
    >
      <p className="text-fg-muted text-metadata">
        Other browsers where you are signed in are signed out. Your apps stay connected.
      </p>
      <Field
        label="Current password"
        className="mt-5"
        type="password"
        value={current}
        onChange={(event) => setCurrent(event.target.value)}
        autoComplete="current-password"
        required
      />
      <NewPasswordField
        label="New password"
        className="mt-4"
        value={next}
        onChange={(event) => setNext(event.target.value)}
        required
      />
    </Form>
  );
}

/** Who is signed in, their password, and the way out. */
export function AccountSection() {
  const me = useMe().data;
  const signOut = useSignOut();
  const [changing, setChanging] = useState(false);
  const [changed, setChanged] = useState(false);

  return (
    <Section title="Account">
      <dl>
        <Row label="Name">{me?.name}</Row>
        <Row label="Role">{me?.role === "owner" ? "Owner" : "Member"}</Row>
      </dl>
      {changed ? (
        <p role="status" className="mt-2 text-metadata text-success">
          Your password was changed.
        </p>
      ) : null}
      {signOut.isError ? <ErrorText className="mt-2">{signOut.error.message}</ErrorText> : null}
      <div className="mt-4 flex flex-wrap gap-2">
        <Button
          onClick={() => {
            setChanged(false);
            setChanging(true);
          }}
        >
          Change password
        </Button>
        <Button variant="quiet" disabled={signOut.isPending} onClick={() => signOut.mutate()}>
          Sign out
        </Button>
      </div>

      <Dialog open={changing} onClose={() => setChanging(false)} title="Change your password">
        <PasswordForm
          onClose={() => setChanging(false)}
          onChanged={() => {
            setChanging(false);
            setChanged(true);
          }}
        />
      </Dialog>
    </Section>
  );
}
