import { useState } from "react";
import { useSetUp } from "../api/account";
import { Button } from "../components/Button";
import { Field } from "../components/Field";
import { Form } from "../components/Form";
import { Entrance } from "./Entrance";
import { NewPasswordField } from "./NewPasswordField";

/** First-run setup: the person who opens a new server makes its owner. */
export function SetupPage() {
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [again, setAgain] = useState("");
  // Said only once the form was sent, not while the second one is being typed.
  const [mismatch, setMismatch] = useState(false);
  const setUp = useSetUp();

  function submit() {
    setMismatch(password !== again);
    if (password === again) {
      setUp.mutate({ name, password });
    }
  }

  return (
    <Entrance
      title="Set up this server"
      lead="Create the owner's account. The owner adds libraries and decides who else may read them."
    >
      <Form
        onSubmit={submit}
        problem={mismatch ? "The two passwords are not the same." : setUp.error?.message}
        actions={
          <Button
            variant="primary"
            type="submit"
            className="mt-6 w-full"
            disabled={setUp.isPending}
          >
            {setUp.isPending ? "Creating…" : "Create the owner"}
          </Button>
        }
      >
        <Field
          label="Name"
          className="mt-5"
          value={name}
          onChange={(event) => setName(event.target.value)}
          autoComplete="username"
          autoCapitalize="off"
          spellCheck={false}
          autoFocus
          required
        />
        <NewPasswordField
          className="mt-4"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          required
        />
        <Field
          label="Password again"
          className="mt-4"
          type="password"
          value={again}
          onChange={(event) => setAgain(event.target.value)}
          autoComplete="new-password"
          required
        />
      </Form>
    </Entrance>
  );
}
