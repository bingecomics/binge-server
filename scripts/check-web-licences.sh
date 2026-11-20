#!/usr/bin/env bash
# Fails when a package that ships in the web app's bundle has a licence that
# is not known to go with the server's own, the AGPL. The Rust crates have
# the same check in deny.toml.
#
# Needs the web app's packages installed: pnpm -C web install
set -euo pipefail
cd "$(dirname "$0")/.."

if [ "$#" -gt 0 ]; then
  echo "scripts/check-web-licences.sh takes no arguments." >&2
  exit 2
fi

# shellcheck disable=SC2016  # what is in the quotes is JavaScript
pnpm -C web licenses list --prod --json | node -e '
  // Add a licence only after making sure it goes with the AGPL.
  const allowed = new Set([
    "0BSD", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "CC0-1.0", "ISC", "MIT", "MPL-2.0",
    "Unlicense", "Zlib",
  ]);
  let text = "";
  process.stdin.on("data", (part) => (text += part));
  process.stdin.on("end", () => {
    const byLicence = JSON.parse(text);
    const packages = Object.values(byLicence).flat().length;
    if (packages === 0) {
      console.error("No packages found. Run: pnpm -C web install");
      process.exit(1);
    }
    const refused = Object.entries(byLicence).filter(([licence]) => !allowed.has(licence));
    for (const [licence, under] of refused) {
      const names = under.map((one) => one.name).join(", ");
      console.error(`Not known to go with the AGPL: "${licence}" (${names})`);
    }
    if (refused.length > 0) process.exit(1);
    console.log(`The ${packages} packages in the web bundle have licences that go with the AGPL.`);
  });
'
