//! What was read: the caller's own reading looked back on, and for the
//! owner who is reading now.

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::{Caller, Owner};
use super::browse::Book;
use super::error::ApiError;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(reading))
        .routes(routes!(reading_now))
}

/// A day the caller read.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadingDay {
    /// The day on the caller's clock, as `2026-11-18`.
    pub day: String,
    /// Books read from.
    pub books: i64,
    /// Pages gone through.
    pub pages: i64,
    /// Time spent reading, in seconds.
    pub seconds: i64,
}

/// A month the caller read in.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadingMonth {
    /// From 1, January.
    pub month: i64,
    pub books: i64,
    pub pages: i64,
    pub seconds: i64,
}

/// A writer the caller read.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadWriter {
    /// The person.
    pub id: i64,
    pub name: String,
    /// Books of theirs read from.
    pub books: i64,
    /// Pages of those books gone through.
    pub pages: i64,
}

/// What the caller read in one year. It is told from their reading itself,
/// a stretch of pages at a time: marking a book read adds nothing to it.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadingYear {
    pub year: i64,
    /// The first year the caller read in. Missing when they never read.
    pub first_year: Option<i64>,
    /// The days they read, in order. A day without reading is left out.
    pub days: Vec<ReadingDay>,
    /// The months they read in, in order.
    pub months: Vec<ReadingMonth>,
    /// The ten writers they read most, the one with most books first.
    pub writers: Vec<ReadWriter>,
    /// Books read from over the whole year.
    pub books: i64,
    pub pages: i64,
    pub seconds: i64,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ReadingQuery {
    /// The year to look back on, on the caller's clock.
    pub year: i64,
    /// How far the caller's clock is ahead of UTC, in minutes: `-180` three
    /// hours behind. It says where their days start. UTC when left out.
    #[serde(default)]
    pub minutes_ahead: i64,
}

/// Tells what the caller read in a year: the days they read, the books
/// and pages of each month, and the writers they read most.
#[utoipa::path(
    get,
    path = "/me/reading",
    tag = "reading",
    operation_id = "getReadingYear",
    params(ReadingQuery),
    responses(
        (status = OK, body = ReadingYear),
        (status = BAD_REQUEST, description = "The year or the clock is not one", body = ApiError),
    )
)]
async fn reading(
    State(state): State<AppState>,
    caller: Caller,
    Query(query): Query<ReadingQuery>,
) -> Result<Json<ReadingYear>, ApiError> {
    let year = state
        .catalog
        .reading_in(caller.user.id, query.year, query.minutes_ahead)
        .await?;
    Ok(Json(ReadingYear {
        year: year.year,
        first_year: year.first_year,
        days: year
            .days
            .into_iter()
            .map(|day| ReadingDay {
                day: day.day,
                books: day.books,
                pages: day.pages,
                seconds: day.seconds,
            })
            .collect(),
        months: year
            .months
            .into_iter()
            .map(|month| ReadingMonth {
                month: month.month,
                books: month.books,
                pages: month.pages,
                seconds: month.seconds,
            })
            .collect(),
        writers: year
            .writers
            .into_iter()
            .map(|writer| ReadWriter {
                id: writer.id,
                name: writer.name,
                books: writer.books,
                pages: writer.pages,
            })
            .collect(),
        books: year.books,
        pages: year.pages,
        seconds: year.seconds,
    }))
}

/// Someone reading now.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadingNow {
    pub user_id: i64,
    /// What the person signs in with.
    pub user_name: String,
    pub book: Book,
    /// The page they were shown last, from 0.
    pub page: i64,
    /// When, in seconds since 1970.
    pub at: i64,
}

/// Lists who is reading now, the one who turned a page last first: everyone
/// whose last page was shown within the last half hour, each with the book
/// they are in. Only the owner can ask.
#[utoipa::path(
    get,
    path = "/reading-now",
    tag = "reading",
    operation_id = "listReadingNow",
    responses(
        (status = OK, body = [ReadingNow]),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
    )
)]
async fn reading_now(
    State(state): State<AppState>,
    Owner(owner): Owner,
) -> Result<Json<Vec<ReadingNow>>, ApiError> {
    let reading = state.catalog.reading_now(owner.id).await?;
    Ok(Json(
        reading
            .into_iter()
            .map(|one| ReadingNow {
                user_id: one.user_id.0,
                user_name: one.user_name,
                book: one.book.into(),
                page: one.page,
                at: one.at,
            })
            .collect(),
    ))
}
