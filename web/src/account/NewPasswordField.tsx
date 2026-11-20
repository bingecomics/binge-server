import type { ComponentProps } from "react";
import { Field } from "../components/Field";

/** The fewest characters the server takes for a password. */
const MIN_PASSWORD = 8;

/** What to say about a password where one is chosen. */
export const PASSWORD_HINT = `At least ${MIN_PASSWORD} characters.`;

/** A field in which a password is chosen, by whoever will use it or for them. */
export function NewPasswordField({
  label = "Password",
  hint = PASSWORD_HINT,
  ...input
}: Partial<ComponentProps<typeof Field>>) {
  return (
    <Field
      label={label}
      hint={hint}
      type="password"
      autoComplete="new-password"
      minLength={MIN_PASSWORD}
      {...input}
    />
  );
}
