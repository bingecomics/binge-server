//! People: who the books name, their series and books, and the names they
//! go by.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::{Caller, Owner};
use super::browse::{PagedBooks, PartQuery, Series};
use super::error::ApiError;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(get, show_by))
        .routes(routes!(series))
        .routes(routes!(books))
        .routes(routes!(merge))
        .routes(routes!(add_name))
        .routes(routes!(split))
        .routes(routes!(credits))
}

/// Someone books name for a job. `GET /people/{id}` says more about them.
#[derive(Debug, Serialize, ToSchema)]
pub struct Credit {
    /// The person.
    pub id: i64,
    /// The name they are shown by, whichever way the books spell it.
    pub name: String,
}

impl From<binge_core::Credited> for Credit {
    fn from(credited: binge_core::Credited) -> Self {
        Self {
            id: credited.id,
            name: credited.name,
        }
    }
}

/// Someone the series of a library can be narrowed by.
#[derive(Debug, Serialize, ToSchema)]
pub struct NamedPerson {
    /// The person.
    pub id: i64,
    pub name: String,
    /// How many series of the library have a book that names them.
    pub series_count: i64,
}

impl From<binge_core::NamedPerson> for NamedPerson {
    fn from(person: binge_core::NamedPerson) -> Self {
        Self {
            id: person.id,
            name: person.name,
            series_count: person.series_count,
        }
    }
}

/// A person the books name as a writer or a penciller, counted over the
/// books the caller may read. Someone none of those books names is not
/// there for the caller.
///
/// Files spell one person several ways, so a person is the names they go
/// by. Two spellings that differ only in capitals, accents, spaces or
/// punctuation are one name from the start; the owner makes the rest one.
#[derive(Debug, Serialize, ToSchema)]
pub struct Person {
    pub id: i64,
    /// The name they are shown by: one of their names.
    pub name: String,
    /// Books that name them, for whatever job.
    pub book_count: i64,
    /// Books that name them as a writer.
    pub written_count: i64,
    /// Books that name them as a penciller.
    pub pencilled_count: i64,
    /// Series that hold those books.
    pub series_count: i64,
}

impl From<binge_core::Person> for Person {
    fn from(person: binge_core::Person) -> Self {
        Self {
            id: person.id,
            name: person.name,
            book_count: person.book_count,
            written_count: person.written_count,
            pencilled_count: person.pencilled_count,
            series_count: person.series_count,
        }
    }
}

/// A part of a list of people.
#[derive(Debug, Serialize, ToSchema)]
pub struct PagedPeople {
    pub people: Vec<Person>,
    /// How many people the whole list holds.
    pub total: i64,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PeopleQuery {
    /// Words to find people by: every one of them in one of their names.
    /// Case and accents do not matter. Left out or empty, everyone.
    #[serde(default)]
    pub q: String,
}

/// Lists a part of the people the books the caller may read name, the ones
/// most books name first, with how many there are in all. With `q`, only
/// those one of whose names holds every word of it: the way to find the
/// two spellings of one person before making them one.
#[utoipa::path(
    get,
    path = "/people",
    tag = "people",
    operation_id = "listPeople",
    params(PeopleQuery, PartQuery),
    responses((status = OK, body = PagedPeople))
)]
async fn list(
    State(state): State<AppState>,
    caller: Caller,
    Query(query): Query<PeopleQuery>,
    Query(part): Query<PartQuery>,
) -> Result<Json<PagedPeople>, ApiError> {
    let found = state.catalog.people(caller.user.id, &query.q, part.span());
    let part = found.await?;
    Ok(Json(PagedPeople {
        people: part.items.into_iter().map(Person::from).collect(),
        total: part.total,
    }))
}

/// One way of writing a person's name.
#[derive(Debug, Serialize, ToSchema)]
pub struct PersonName {
    pub id: i64,
    pub name: String,
}

/// A person with every name they go by.
#[derive(Debug, Serialize, ToSchema)]
pub struct PersonDetails {
    #[serde(flatten)]
    pub person: Person,
    /// Their names, the one they are shown by first.
    pub names: Vec<PersonName>,
}

/// A person as `user` sees them.
async fn details(
    state: &AppState,
    user: &binge_core::User,
    id: i64,
) -> Result<Json<PersonDetails>, ApiError> {
    let (person, names) = state.catalog.person_with_names(user.id, id).await?;
    Ok(Json(PersonDetails {
        person: person.into(),
        names: names
            .into_iter()
            .map(|name| PersonName {
                id: name.id,
                name: name.name,
            })
            .collect(),
    }))
}

/// Returns one person with the names they go by.
#[utoipa::path(
    get,
    path = "/people/{id}",
    tag = "people",
    operation_id = "getPerson",
    params(("id" = i64, Path, description = "The person")),
    responses((status = OK, body = PersonDetails), (status = NOT_FOUND, body = ApiError))
)]
async fn get(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<PersonDetails>, ApiError> {
    details(&state, &caller.user, id).await
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PersonChange {
    /// The id of the one of their names to show them by.
    pub shown_by: i64,
}

/// Shows a person by another of their names. Only the owner can.
#[utoipa::path(
    patch,
    path = "/people/{id}",
    tag = "people",
    operation_id = "changePerson",
    params(("id" = i64, Path, description = "The person")),
    request_body = PersonChange,
    responses(
        (status = OK, body = PersonDetails),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, description = "No such person, or the name is not theirs", body = ApiError),
    )
)]
async fn show_by(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(change): Json<PersonChange>,
) -> Result<Json<PersonDetails>, ApiError> {
    state.catalog.show_person_by(id, change.shown_by).await?;
    details(&state, &owner, id).await
}

/// Lists the series that hold a book that names the person, in name order.
#[utoipa::path(
    get,
    path = "/people/{id}/series",
    tag = "people",
    operation_id = "listSeriesOfPerson",
    params(("id" = i64, Path, description = "The person")),
    responses((status = OK, body = [Series]), (status = NOT_FOUND, body = ApiError))
)]
async fn series(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Series>>, ApiError> {
    let series = state.catalog.series_of_person(caller.user.id, id).await?;
    Ok(Json(series.into_iter().map(Series::from).collect()))
}

/// Lists a part of the books that name the person, from the first that
/// came out, with how many there are in all. The ones that say no date come
/// last, by series and number.
#[utoipa::path(
    get,
    path = "/people/{id}/books",
    tag = "people",
    operation_id = "listBooksOfPerson",
    params(("id" = i64, Path, description = "The person"), PartQuery),
    responses((status = OK, body = PagedBooks), (status = NOT_FOUND, body = ApiError))
)]
async fn books(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
    Query(part): Query<PartQuery>,
) -> Result<Json<PagedBooks>, ApiError> {
    let books = state
        .catalog
        .books_of_person(caller.user.id, id, part.span())
        .await?;
    Ok(Json(books.into()))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PeopleToMerge {
    /// The ids of the people to make one with this one: 50 at most.
    pub people: Vec<i64>,
}

/// Makes several people one. The people named are gone and their names
/// become names of this one, who keeps the name they are shown by. No file
/// is touched, and `DELETE /people/{id}/names/{name}` takes a name back
/// out. Only the owner can.
#[utoipa::path(
    post,
    path = "/people/{id}/merge",
    tag = "people",
    operation_id = "mergePeople",
    params(("id" = i64, Path, description = "The person who stays")),
    request_body = PeopleToMerge,
    responses(
        (status = OK, body = PersonDetails),
        (status = BAD_REQUEST, description = "Nobody is named, or the person themself", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, description = "One of the people is not there", body = ApiError),
    )
)]
async fn merge(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(others): Json<PeopleToMerge>,
) -> Result<Json<PersonDetails>, ApiError> {
    state.catalog.merge_people(id, &others.people).await?;
    details(&state, &owner, id).await
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewPersonName {
    /// One more way of writing the person's name.
    pub name: String,
}

/// Gives a person one more name, for the files that will spell them so. A
/// name that is already theirs changes nothing. Only the owner can.
#[utoipa::path(
    post,
    path = "/people/{id}/names",
    tag = "people",
    operation_id = "addPersonName",
    params(("id" = i64, Path, description = "The person")),
    request_body = NewPersonName,
    responses(
        (status = OK, body = PersonDetails),
        (status = BAD_REQUEST, description = "The name is blank or too long", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, description = "Someone else goes by the name: `conflicts_with` is who", body = ApiError),
    )
)]
async fn add_name(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(new): Json<NewPersonName>,
) -> Result<Json<PersonDetails>, ApiError> {
    state.catalog.name_person(id, &new.name).await?;
    details(&state, &owner, id).await
}

/// Makes one of a person's names a person of its own again, and answers
/// with that person. The books that spell the name so follow it. The name
/// a person is shown by cannot leave them. Only the owner can.
#[utoipa::path(
    delete,
    path = "/people/{id}/names/{name}",
    tag = "people",
    operation_id = "splitPerson",
    params(
        ("id" = i64, Path, description = "The person"),
        ("name" = i64, Path, description = "The id of the name"),
    ),
    responses(
        (status = OK, body = Credit),
        (status = BAD_REQUEST, description = "It is the name they are shown by", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, description = "No such person, or the name is not theirs", body = ApiError),
    )
)]
async fn split(
    State(state): State<AppState>,
    _: Owner,
    Path((id, name)): Path<(i64, i64)>,
) -> Result<Json<Credit>, ApiError> {
    let apart = state.catalog.split_person(id, name).await?;
    Ok(Json(apart.into()))
}

/// Who a book names.
#[derive(Debug, Serialize, ToSchema)]
pub struct BookCredits {
    pub writers: Vec<Credit>,
    pub pencillers: Vec<Credit>,
}

/// Returns who a book names as its writers and pencillers, as people: the
/// names in the book's `info` are only what its file spells.
#[utoipa::path(
    get,
    path = "/books/{id}/credits",
    tag = "people",
    operation_id = "getBookCredits",
    params(("id" = i64, Path, description = "The book")),
    responses((status = OK, body = BookCredits), (status = NOT_FOUND, body = ApiError))
)]
async fn credits(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<BookCredits>, ApiError> {
    let credits = state.catalog.credits_of(caller.user.id, id).await?;
    Ok(Json(BookCredits {
        writers: credits.writers.into_iter().map(Credit::from).collect(),
        pencillers: credits.pencillers.into_iter().map(Credit::from).collect(),
    }))
}
