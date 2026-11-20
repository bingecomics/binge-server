/**
 * Asks the person for a file of the kinds `accept` names. The field that
 * asks is nobody's: a menu that asked is gone before the person has
 * answered.
 */
export function askForFile(accept: string, given: (file: File) => void) {
  const field = document.createElement("input");
  field.type = "file";
  field.accept = accept;
  field.hidden = true;
  field.addEventListener("change", () => {
    const file = field.files?.[0];
    if (file) {
      given(file);
    }
    field.remove();
  });
  field.addEventListener("cancel", () => field.remove());
  document.body.append(field);
  field.click();
}
