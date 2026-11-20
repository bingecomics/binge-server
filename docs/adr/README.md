# Decision records

One short file per decision that shapes the product or the code and would be
expensive to reverse. Each says what was decided, why, and what follows from
it, so nobody has to guess later or reopen it by accident.

| | Decision | Status |
| --- | --- | --- |
| [0001](0001-free-local-server-for-binge-clients-only.md) | Free local server, for Binge's own clients only | Accepted |
| [0002](0002-ai-work-runs-in-the-users-ai-client.md) | AI work runs in the user's AI client | Accepted |
| [0003](0003-one-binary-rust-sqlite-embedded-ui.md) | One binary: Rust, SQLite, embedded web UI | Accepted |
| [0004](0004-identify-books-by-page-content.md) | Identify books by page content | Accepted, amended 2026-11-17 |
| [0005](0005-library-folders-are-read-only.md) | Library folders are read-only | Accepted |
| [0006](0006-one-api-contract-generated-from-code.md) | One API contract, generated from the code | Accepted |
| [0007](0007-refuse-unknown-host-names.md) | Refuse requests for unknown host names | Accepted |
| [0008](0008-one-reader-until-accounts-exist.md) | One reader until accounts exist | Superseded by 0009 |
| [0009](0009-first-run-setup-makes-the-owner.md) | First-run setup makes the owner; members read what they are given | Accepted |
| [0010](0010-sessions-for-browsers-tokens-for-apps.md) | Sessions for browsers, tokens for apps | Accepted |
| [0011](0011-read-cbr-and-pdf-with-rust-libraries.md) | Read CBR and PDF with libraries written in Rust | Accepted |
| [0012](0012-series-and-groups-are-a-layout-over-the-files.md) | Series and groups are a layout over the files | Accepted, amended 2026-11-17 |
| [0013](0013-the-layout-changes-only-as-asked.md) | The layout changes only as asked | Accepted |
| [0014](0014-reading-lists-belong-to-the-server.md) | Reading lists belong to the server | Superseded by 0021 |
| [0015](0015-long-lists-are-read-a-part-at-a-time.md) | Long lists are read a part at a time | Accepted |
| [0016](0016-a-person-is-the-names-they-go-by.md) | A person is the names they go by | Accepted |
| [0017](0017-a-view-is-a-library-narrowed-and-kept-by-name.md) | A view is a library narrowed, kept by name | Accepted |
| [0018](0018-what-a-person-chooses-is-kept-as-keys-and-values.md) | What a person chooses is kept as keys and values | Accepted |
| [0019](0019-the-server-is-under-the-agpl.md) | The server is under the AGPL | Accepted |
| [0020](0020-the-server-updates-itself-from-signed-releases.md) | The server updates itself from signed releases | Accepted |
| [0021](0021-a-reading-list-is-its-makers-to-share.md) | A reading list is its maker's, to share | Accepted |

## Adding one

- Take the next number and name the file after the decision, not the problem.
- Keep the four headings: Status, Context, Decision, Consequences. Add
  "Alternatives considered" when a real alternative was turned down.
- Do not rewrite an accepted record when the decision changes. Write a new
  one and mark the old one "Superseded by NNNN".
- Add the row to the table above and to the list in
  [architecture.md](../architecture.md).

Small choices that are cheap to change (a library, a lint rule, a folder
name) do not need a record.
