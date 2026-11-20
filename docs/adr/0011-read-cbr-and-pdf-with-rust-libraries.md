# 0011. Read CBR and PDF with libraries written in Rust

## Status

Accepted on 2026-11-17. Built in iteration 4.

## Context

Comic collections are not only CBZ. Many books are CBR (a rar archive of
images) and some are PDF, mostly scans where every page is one picture. A
server that skips them shows a library with holes in it.

Two things made the choice of how to read them matter:

- **One binary.** The server installs as "download, run, open the browser"
  ([0003](0003-one-binary-rust-sqlite-embedded-ui.md)). The usual way to
  read a PDF is PDFium, a large C++ library shipped as a separate file for
  each system. The usual way to read a rar is the unrar source, which is C++
  and has to be compiled for each system.
- **The licence.** The server is to be open source. The unrar source is free
  to use but its licence is not an open-source one, which would limit the
  licence the server itself can take.

Pages are otherwise served exactly as the file stores them. A rar keeps
images as files, like a zip. A PDF does not: a page is a set of drawing
instructions, which may or may not come down to one picture.

## Decision

- **CBR is read with `rars`**, a rar reader written in Rust under
  Apache-2.0, built without its writing half. Tests use the writing half to
  make their own archives. Its decryption is built in only so that a
  protected archive can be told from a broken one.
- **PDF is read with `hayro`**, a PDF reader and renderer written in Rust
  under Apache-2.0 or MIT.
- **A file is what its first bytes say.** `PK` is a zip, `Rar!` a rar,
  `%PDF` a PDF. The name decides only when the bytes do not. Files named
  `.cbr` that are zips are common.
- **A scanned PDF page is handed out untouched.** When a page draws one
  JPEG and nothing else, upright, in its own proportions, in plain grey or
  red-green-blue and without a mask, the picture alone looks as the page
  does, and the server sends those bytes.
- **Any other PDF page is drawn**, on white, 2200 pixels on its longer
  side, and sent as a JPEG of quality 90.
- **A protected file is not opened.** There is nowhere to type a password.
  It is listed as a book that cannot be read, and says why.
- **The Rust version is pinned** in `rust-toolchain.toml`, since `hayro`
  needs a newer compiler than the project started with.

## Consequences

- Nothing is installed beside the binary, on any system, and nothing has to
  be compiled from C or C++.
- The licence question about rar is gone. The server can take any
  open-source licence.
- Both libraries are younger than what they replace. They read every CBR
  and PDF of the library this was built against, with each unpacked page
  matching the checksum its archive keeps, but a file they cannot read will
  turn up. Such a file is listed with what is wrong with it; it does not
  stop the scan.
- A PDF is read into memory whole when it is opened, and it is opened for
  each page asked for. A PDF of several hundred megabytes costs that much
  memory for the time of a request.
- A drawn page is the one place where the server makes an image instead of
  passing one on. How it looks depends on the drawing library, so it can
  change a little between versions of the server, while the page's address
  stays the same and browsers keep what they had.
- In a rar packed as one stream (a "solid" archive), reaching a page means
  unpacking the ones before it, so scanning and reading such a book gets
  slower the more pages it has. Comic archives are rarely packed that way.
- A scan that also carries text, as after text recognition, is not seen as
  one picture. Its pages are drawn: correct, but slower and re-encoded.
- A file that makes either library crash is listed as a book that cannot be
  read. The crash is caught; it does not stop the scan or the server.
- The binary grows by the size of a PDF renderer.

## Alternatives considered

- **PDFium for PDF.** Draws everything, and is what browsers use. It is a
  separate library file for each system, which breaks the one binary.
- **The unrar source for CBR.** Reads every rar there is. Its licence is
  not open source, and it is C++ to compile for each system.
- **Running `unrar`, `7z` or `pdftoppm` if they are installed.** Nothing to
  build, but the server would work or not depending on what else is on the
  machine, and a NAS usually has none of them.
- **Reading only the pictures of a PDF, never drawing.** Enough for scans
  and much smaller. A PDF made any other way would show blank or wrong
  pages with nothing to say why.
- **Drawing every PDF page.** One path instead of two. It would decode and
  re-encode scans, losing quality and time for pages that are already the
  JPEG to send.
