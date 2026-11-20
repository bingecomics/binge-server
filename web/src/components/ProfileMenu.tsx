import { BookMarked, CalendarDays, ChevronDown, KeyRound, LogOut, UserRound } from "lucide-react";
import { useMe, useSignOut } from "../api/account";
import { Menu, MenuItem, MenuLink } from "./Menu";

/**
 * Who is signed in, at the end of the bar at the top. It opens what is the
 * person's own: the pages they kept, what they read, their account, their
 * tokens, and the way out.
 */
export function ProfileMenu() {
  const me = useMe().data;
  const signOut = useSignOut();

  if (!me) {
    return null;
  }
  return (
    <Menu
      label={`${me.name}: your account`}
      title={me.name}
      className="flex min-h-11 items-center gap-2 rounded-control px-1.5 text-metadata transition-colors duration-150 hover:bg-raised sm:pr-2"
      button={
        <>
          <span
            className="grid size-8 shrink-0 place-items-center rounded-full bg-raised font-semibold uppercase ring-1 ring-outline"
            aria-hidden="true"
          >
            {me.name.trim().charAt(0)}
          </span>
          <span className="max-w-32 truncate font-semibold max-sm:hidden">{me.name}</span>
          <ChevronDown className="size-4 text-fg-muted max-sm:hidden" aria-hidden="true" />
        </>
      }
    >
      <div className="px-3 pt-1 pb-2">
        <p className="truncate font-semibold">{me.name}</p>
        <p className="text-caption text-fg-muted">{me.role === "owner" ? "Owner" : "Member"}</p>
      </div>
      <div className="border-divider border-t pt-2">
        <MenuLink to="/kept" icon={BookMarked}>
          Kept pages
        </MenuLink>
        <MenuLink to="/history" icon={CalendarDays}>
          What you read
        </MenuLink>
        <MenuLink to="/settings/account" icon={UserRound}>
          Account
        </MenuLink>
        <MenuLink to="/settings/tokens" icon={KeyRound}>
          App tokens
        </MenuLink>
        <MenuItem icon={LogOut} onSelect={() => signOut.mutate()} disabled={signOut.isPending}>
          Sign out
        </MenuItem>
      </div>
    </Menu>
  );
}
