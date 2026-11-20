import { useState } from "react";
import { useSignIn } from "../api/account";
import { Button } from "../components/Button";
import { Field } from "../components/Field";
import { Form } from "../components/Form";
import { Entrance } from "./Entrance";

export function SignInPage() {
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const signIn = useSignIn();

  return (
    <Entrance title="Sign in">
      <Form
        onSubmit={() => signIn.mutate({ name, password })}
        problem={signIn.error?.message}
        actions={
          <Button
            variant="primary"
            type="submit"
            className="mt-6 w-full"
            disabled={signIn.isPending}
          >
            {signIn.isPending ? "Signing in…" : "Sign in"}
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
        <Field
          label="Password"
          className="mt-4"
          type="password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          autoComplete="current-password"
          required
        />
      </Form>
    </Entrance>
  );
}
