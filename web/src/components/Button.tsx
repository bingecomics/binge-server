import type { ButtonHTMLAttributes } from "react";

const base =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-control px-4 font-semibold " +
  "text-metadata transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50";

const variants = {
  primary: "bg-accent text-on-accent hover:brightness-110",
  secondary: "bg-raised text-fg hover:bg-divider",
  danger: "bg-danger text-background hover:brightness-110",
  quiet: "text-fg-muted hover:bg-raised hover:text-fg",
};

/** How a button looks, for a link that is to look like one. */
export function buttonClass(variant: keyof typeof variants = "secondary"): string {
  return `${base} ${variants[variant]}`;
}

/**
 * How a link looks that is one of a few choices of what a page shows. The
 * one marked as current is the one chosen.
 */
export const choiceClass = `${buttonClass()} aria-[current]:bg-fg aria-[current]:text-background`;

type Props = ButtonHTMLAttributes<HTMLButtonElement> & { variant?: keyof typeof variants };

export function Button({ variant = "secondary", className = "", type = "button", ...rest }: Props) {
  return <button type={type} className={`${buttonClass(variant)} ${className}`} {...rest} />;
}
