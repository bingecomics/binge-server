import createClient from "openapi-fetch";
import { report } from "../problems";
import type { components, paths } from "./schema";

/** Typed client for the server's API. Types come from the OpenAPI description. */
export const api = createClient<paths>({ baseUrl: "/" });

type Schemas = components["schemas"];
export type Library = Schemas["Library"];
export type ScanStatus = Schemas["ScanStatus"];
export type Series = Schemas["Series"];
export type Group = Schemas["Group"];
export type BooksOverview = Schemas["BooksOverview"];
export type ReadSoFar = Schemas["ReadSoFar"];
export type Book = Schemas["Book"];
export type Page = Schemas["Page"];
export type ReadMark = Schemas["ReadMark"];
export type ReadingList = Schemas["ReadingList"];
export type NewReadingList = Schemas["NewReadingList"];
export type ReadingListDetails = Schemas["ReadingListDetails"];
export type Sharing = Schemas["Sharing"];
export type UserName = Schemas["UserName"];
export type CblPlan = Schemas["CblPlan"];
export type CblLine = Schemas["CblLine"];
export type CblEntry = Schemas["CblEntry"];
export type Person = Schemas["Person"];
export type PersonDetails = Schemas["PersonDetails"];
export type Credit = Schemas["Credit"];
export type NamedPerson = Schemas["NamedPerson"];
export type LibraryChoices = Schemas["LibraryChoices"];
export type Letter = Schemas["Letter"];
export type SearchResults = Schemas["SearchResults"];
export type HeldTwice = Schemas["HeldTwice"];
export type NewLibrary = Schemas["NewLibrary"];
export type Health = Schemas["Health"];
export type User = Schemas["User"];
export type Credentials = Schemas["Credentials"];
export type ApiToken = Schemas["ApiToken"];
export type NewUser = Schemas["NewUser"];
export type UserChange = Schemas["UserChange"];
export type View = Schemas["View"];
export type NewView = Schemas["NewView"];
export type ViewChange = Schemas["ViewChange"];
export type ReadingYear = Schemas["ReadingYear"];
export type ScanRecord = Schemas["ScanRecord"];
export type ScanChange = Schemas["ScanChange"];
export type ChangeKind = Schemas["ChangeKind"];
export type DatabaseCopies = Schemas["DatabaseCopies"];
export type UpdateStatus = Schemas["UpdateStatus"];
export type KeptPage = Schemas["KeptPage"];
export type Followed = Schemas["Followed"];

function isApiError(value: unknown): value is Schemas["ApiError"] {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { message?: unknown }).message === "string"
  );
}

/** A request the server answered with an error. Its message is the server's own sentence. */
export class ApiProblem extends Error {
  /** The HTTP status of the answer. */
  readonly status: number;
  /** What kind of error it is. Missing when the answer did not come from the server's API. */
  readonly code?: Schemas["ErrorCode"];

  constructor(message: string, status: number, code?: Schemas["ErrorCode"]) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

/**
 * Returns what an API call answered, or throws an `ApiProblem` whose message
 * says what went wrong in the server's own words.
 */
export function unwrap<T>(result: { data?: T; error?: unknown; response: Response }): T {
  const { response, error } = result;
  if (!response.ok) {
    throw isApiError(error)
      ? new ApiProblem(error.message, response.status, error.code)
      : new ApiProblem(`The server answered ${response.status}.`, response.status);
  }
  return result.data as T;
}

/** Whether a request was refused because nobody is signed in, or not any more. */
export function isSignedOut(error: unknown): boolean {
  return error instanceof ApiProblem && error.code === "unauthorized";
}

/** What a proxy in front of the server answers when the server does not. */
const NO_SERVER_BEHIND = [502, 503, 504];

/**
 * Whether a request failed because the server did not answer it: `fetch`
 * then fails with a TypeError, or a proxy answers in the server's place.
 */
export function gotNoAnswer(error: unknown): boolean {
  return (
    error instanceof TypeError ||
    (error instanceof ApiProblem && NO_SERVER_BEHIND.includes(error.status))
  );
}

/** What to say when a request got no answer at all. */
const NOT_ANSWERING = "The server is not answering.";

/** What went wrong with a request, as a sentence: the server's own when it sent one. */
export function reason(error: unknown): string {
  return error instanceof Error && !gotNoAnswer(error) ? error.message : NOT_ANSWERING;
}

/** Says what went wrong over the page, for what has no place of its own to say it. */
export function sayProblem(error: unknown) {
  report(reason(error));
}

/** Address of a cover image, from the `cover` of a series or book. */
export function coverUrl(cover: string): string {
  return `/api/v1/covers/${cover}`;
}

/** Address of a page image, from a book's `identity` and the page's number. */
export function pageUrl(identity: string, number: number): string {
  return `/api/v1/pages/${identity}/${number}`;
}

/** Address of a page as a small picture, for a strip of pages or a card of one. */
export function smallPageUrl(identity: string, number: number): string {
  return `${pageUrl(identity, number)}/thumbnail`;
}

/**
 * A file as the body of a request, sent as it is. One of more than
 * `biggest` megabytes is refused here and not by the server, which would be
 * sent all of it first. `what` names it in the sentence: "picture", "file".
 */
export function fileBody(file: File, what: string, biggest: number, contentType: string) {
  if (file.size > biggest * 1024 * 1024) {
    throw new Error(`That ${what} is too big. The server takes one of ${biggest} MB at most.`);
  }
  return {
    // The description of the API calls a file's bytes numbers.
    body: file as unknown as number[],
    bodySerializer: () => file,
    headers: { "Content-Type": contentType },
  };
}
