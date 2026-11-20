import { Check, Copy } from "lucide-react";
import { useRef, useState } from "react";
import { useCreateToken, useRevokeToken, useTokens } from "../api/account";
import type { ApiToken } from "../api/client";
import { Button } from "../components/Button";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { Dialog, DialogActions } from "../components/Dialog";
import { Field } from "../components/Field";
import { Form } from "../components/Form";
import { ListRow, Section } from "../components/Section";
import { ErrorText } from "../components/text";
import { moment } from "../format";

/**
 * Copies a text from where the page shows it, and says whether it did.
 *
 * A browser gives a page its clipboard only over HTTPS or on localhost, and
 * a server at home is opened by its address over plain HTTP. There the text
 * is selected where it shows and copied as the person's own selection. It
 * stays selected, so that copying by hand is one key away if that fails too.
 */
async function copyShown(text: string, shown: HTMLElement | null): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // No clipboard on this page, or the browser kept it.
  }
  if (!shown) {
    return false;
  }
  window.getSelection()?.selectAllChildren(shown);
  try {
    return document.execCommand("copy");
  } catch {
    return false;
  }
}

/** A token's secret, shown the one time the server knows it. */
function Secret({ secret, onClose }: { secret: string; onClose: () => void }) {
  // Unset until the person asks for a copy.
  const [copied, setCopied] = useState<boolean>();
  const shown = useRef<HTMLParagraphElement>(null);

  async function copy() {
    setCopied(await copyShown(secret, shown.current));
  }

  return (
    <div className="mt-2">
      <p className="text-fg-muted text-metadata">
        Copy this token into the app now. The server does not keep it and cannot show it again.
      </p>
      <p
        ref={shown}
        className="mt-4 select-all break-all rounded-control border border-outline bg-background p-3 font-mono text-caption"
      >
        {secret}
      </p>
      {copied === false ? (
        <p role="status" className="mt-2 text-caption text-fg-muted">
          This browser did not allow copying. The token above is selected: copy it by hand.
        </p>
      ) : null}
      <div className="mt-6 flex justify-end gap-2">
        <Button onClick={copy}>
          {copied ? (
            <Check className="size-4" aria-hidden="true" />
          ) : (
            <Copy className="size-4" aria-hidden="true" />
          )}
          {copied ? "Copied" : "Copy"}
        </Button>
        <Button variant="primary" onClick={onClose}>
          Done
        </Button>
      </div>
    </div>
  );
}

function NewTokenForm({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState("");
  const create = useCreateToken();

  if (create.data) {
    return <Secret secret={create.data.secret} onClose={onClose} />;
  }

  return (
    <Form
      onSubmit={() => create.mutate(name)}
      problem={create.error?.message}
      className="mt-2"
      actions={
        <DialogActions onCancel={onClose}>
          <Button variant="primary" type="submit" disabled={create.isPending || name.trim() === ""}>
            {create.isPending ? "Creating…" : "Create token"}
          </Button>
        </DialogActions>
      }
    >
      <p className="text-fg-muted text-metadata">
        An app uses a token in place of your password. It reads what you read, and cannot change
        your account.
      </p>
      <Field
        label="Name"
        className="mt-5"
        value={name}
        onChange={(event) => setName(event.target.value)}
        placeholder="Living room iPad"
        hint="Something to know it by when you want to revoke it."
        autoComplete="off"
        required
      />
    </Form>
  );
}

function TokenRow({ token, onRevoke }: { token: ApiToken; onRevoke: () => void }) {
  return (
    <ListRow
      title={token.name}
      detail={
        <>
          Created {moment(token.created_at)} ·{" "}
          {token.last_used_at ? `Last used ${moment(token.last_used_at)}` : "Never used"}
        </>
      }
    >
      <Button variant="quiet" onClick={onRevoke}>
        Revoke
      </Button>
    </ListRow>
  );
}

/** The tokens the signed-in user's apps connect with. */
export function TokensSection() {
  const tokens = useTokens();
  const revoke = useRevokeToken();
  const [creating, setCreating] = useState(false);
  const [revoking, setRevoking] = useState<ApiToken | null>(null);

  function closeRevoke() {
    setRevoking(null);
    revoke.reset();
  }

  return (
    <Section
      title="App tokens"
      action={<Button onClick={() => setCreating(true)}>New token</Button>}
    >
      {tokens.isError ? (
        <ErrorText>{tokens.error.message}</ErrorText>
      ) : tokens.data?.length === 0 ? (
        <p className="text-fg-muted">
          None yet. A token connects an app, such as the Binge reader, to your account.
        </p>
      ) : (
        <ul>
          {tokens.data?.map((token) => (
            <TokenRow key={token.id} token={token} onRevoke={() => setRevoking(token)} />
          ))}
        </ul>
      )}

      <Dialog open={creating} onClose={() => setCreating(false)} title="New app token">
        <NewTokenForm onClose={() => setCreating(false)} />
      </Dialog>
      <ConfirmDialog
        open={revoking !== null}
        onClose={closeRevoke}
        title={revoking ? `Revoke “${revoking.name}”?` : ""}
        confirm="Revoke token"
        confirming="Revoking…"
        isPending={revoke.isPending}
        error={revoke.error}
        onConfirm={() => revoking && revoke.mutate(revoking.id, { onSuccess: closeRevoke })}
      >
        The app that uses this token stops working at once, and has to be connected again with a new
        one.
      </ConfirmDialog>
    </Section>
  );
}
