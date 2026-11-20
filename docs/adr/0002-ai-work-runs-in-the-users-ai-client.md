# 0002. AI work runs in the user's AI client

## Status

Accepted on 2026-11-15, decided by the owner. Nothing of it is built yet; it
starts in iteration 6.

## Context

The features that define Binge need a capable AI model: reading the text on a
page, working out who is speaking, translating with the characters and the
story in mind, suggesting how to organize a library.

Someone has to pay for that. In the reader repository's translation
experiment, reading and translating cost about 5 US cents a page, and about
20 cents a page when the page image was regenerated. A long series is
thousands of pages. Binge cannot carry that cost inside a low-priced
subscription.

Many of the people Binge is for already pay for an AI client such as Claude or
ChatGPT, and those clients can use tools through MCP.

## Decision

The server does no AI inference, and Binge pays for none by default.

- The server owns the files and the breakdown of the problem. It finds the
  regions on a page, keeps the series' characters and glossary, queues the
  work, stores the results, and draws the translated text.
- It exposes each step as an MCP tool. Binge ships the MCP server, and the
  skills or plugin that tell an AI client how to use it.
- The user's own AI client does the reading, the reasoning and the
  translating, and sends results back through the tools.
- Results are stored as tracks next to the book, like subtitle tracks. A
  reader switches between them without any AI being involved.

A hosted option, where Binge runs the AI, may exist later as a separate and
expensive subscription. It is not the default path and nothing depends on it.

## Consequences

- Translation quality depends on the user's client and model. The server's
  job is to make the task small and well described enough that a good client
  does it well.
- Results must be precomputed and stored. A reader cannot wait for an AI
  client while someone turns a page.
- The tools are the product's AI surface. They need the same care as the
  HTTP API: stable names, clear inputs, small outputs.
- Work must be resumable. An AI client may stop halfway through a volume.
- `binge-core` stays free of HTTP so the MCP server can sit on it directly.
- Assistants that run in the cloud cannot reach a server at home. They need
  a relay, which is part of Binge+.

## Alternatives considered

- **Binge calls an AI provider and charges for it.** Turned down as the
  default because of the cost per page above.
- **Run a model on the user's machine.** Not ruled out for narrow steps such
  as finding speech balloons, where small models are good enough. Not
  suitable for translation with story context on the hardware most people run
  a comic server on.
