# 0026. Another server is read once, to bring a person over

## Status

Accepted on 2026-11-21, decided by the owner. The first part is built:
the other server is read and what it said is shown. Making reading, lists
and collections of it comes next.

## Context

People who would use this server run another comic server today. Their
files need no change, because this server reads the folders as they are
([0001](0001-free-local-server-for-binge-clients-only.md)). What they
would lose by switching is everything that is not in the files: what they
have read and when, their reading lists and collections, what they want
to read, the pages they marked, and who else uses the server. Years of
that are a reason to stay where one is.

0001 rules out speaking another server's API: no other client connects
here, and the data model follows no other product. Reading another
server is a different thing. This server is then the client, once, and
the other one's API is asked, not offered.

The owner asked for Kavita first, with the record of when things were
read.

## Decision

A person brings what they have on the server they are leaving. This
server reads the other one with an address and a key the person gives,
and changes nothing there.

- **It is not compatibility.** This server offers none of the other
  one's API and takes none of its clients. It asks that server what it
  holds, as that server's own web app does.
- **Kavita is the first it reads.** Which servers it reads is the
  owner's to say. Each is a module of its own in the server
  (`crates/server/src/bring/`) that turns what that server says into one
  shape the core knows (`Taken`). The core knows no other server.
- **Only when asked.** With the look for a newer version
  ([0020](0020-the-server-updates-itself-from-signed-releases.md)), this
  is the one part of the server that calls an address outside it. It does
  so when someone gives it an address and a key, and at no other time.
- **The key is not kept.** It is used for the reading, sent to that
  server only, and written nowhere: not in the database, not in the log,
  not in an error. The address is kept with what was read, so that
  reading the same server again is known to be that, and what the person
  said of it holds.
- **A key is one person's.** A server shows a key what its person may
  see: their reading, their lists. With the key of someone who manages
  that server, its people are read too, by name and by the libraries they
  read. Another person's reading comes with that person's own key.
- **It is shown before anything is made of it.** What was read is kept as
  it was read, in tables of its own in the data directory, and the first
  thing done with it is a report: the libraries and which of this
  server's they are, how many of the files are books here, what there is
  of reading, lists, collections and people. Nothing of it is reading, a
  list or a collection here until the person says so.
- **A library is matched by the owner.** Two servers rarely see a folder
  under the same path. A library there is taken for the one here with the
  same folder or the same name, and the owner puts that right.
- **A book is matched by its place, or else by its file.** A file there
  is a book here when it sits at the same place under its library's
  folder, or else when it has the same name and size. One that is not
  found is listed, not guessed.
- **Reading keeps what it was.** A stretch of reading the other server
  timed comes as it was timed. One it only knows the day of comes as that
  day, with no time spent, and says nothing about how fast the person
  reads. Which is which is the core's to tell from what was kept, not
  each server's module to make up.

## Consequences

- Switching costs a person an address and a key, and they see what will
  come before it does.
- The server now holds code that follows another product's API. When that
  product changes it, the reading says that it could not read the answer
  and keeps nothing; it does not guess.
- An address given by a person is called by the server. Only the owner
  gives one, for now. When a member brings their own reading, they give a
  key and the address stays the owner's.
- What was read stays in the data directory until it is forgotten, so a
  later version can make more of it without asking for the key again.
- Asking the other server leaves a trace there: it sees a client called
  Binge Server, and newer versions of it note when the key's person was
  last seen.
