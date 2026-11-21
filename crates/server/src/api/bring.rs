//! Bringing what a person has on the server they are leaving.
//!
//! The other server is read with an address and a key, and what it said is
//! shown first: nothing here changes what anyone has read or made on this
//! server, and nothing is changed on the other one. Only the owner can,
//! for now.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use binge_core::CatalogError;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Owner;
use super::error::ApiError;
use crate::app::AppState;
use crate::bring::Source;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status, start, forget))
        .routes(routes!(say_library))
}

#[derive(Deserialize, ToSchema)]
pub struct BringAsked {
    pub from: Source,
    /// The other server's address, as it is opened in a browser.
    #[schema(max_length = 2000)]
    pub address: String,
    /// The caller's key there. It is used to read that server and is not
    /// kept.
    #[schema(max_length = 2000)]
    pub key: String,
}

/// A library of the other server, and how much of it this server holds.
#[derive(Debug, Serialize, ToSchema)]
pub struct BroughtLibrary {
    /// Its id there.
    pub their_id: i64,
    pub name: String,
    /// Its folders, as the other server sees them.
    pub folders: Vec<String>,
    /// The library here that holds the same folder: the one it was taken
    /// for, or the one the owner said.
    pub library_id: Option<i64>,
    /// Its files there.
    pub books: i64,
    /// The ones this server holds at the same place under the folder.
    pub by_path: i64,
    /// The ones it holds somewhere else, told by their name and size.
    pub by_file: i64,
    /// Some of the ones it does not hold: where each is under the folder.
    pub missing: Vec<String>,
}

impl From<binge_core::BroughtLibrary> for BroughtLibrary {
    fn from(library: binge_core::BroughtLibrary) -> Self {
        Self {
            their_id: library.their_id,
            name: library.name,
            folders: library.folders,
            library_id: library.library_id,
            books: library.books,
            by_path: library.by_path,
            by_file: library.by_file,
            missing: library.missing,
        }
    }
}

/// What the caller has read on the other server.
#[derive(Debug, Serialize, ToSchema)]
pub struct BroughtReading {
    /// What they started and did not finish there.
    pub started: i64,
    /// What they finished there.
    pub finished: i64,
    /// How many of those two are books this server holds.
    pub found: i64,
    /// The stretches of reading that server timed.
    pub timed: i64,
    /// The seconds those took together.
    pub seconds: i64,
    /// The reading it only knows the day of.
    pub dated: i64,
    /// When the first and the last stretch started, in seconds since 1970.
    pub first_at: Option<i64>,
    pub last_at: Option<i64>,
}

/// Someone who uses the other server.
#[derive(Debug, Serialize, ToSchema)]
pub struct BroughtPerson {
    pub name: String,
    /// Whether they manage that server.
    pub manages: bool,
    /// Whether they are kept from some of what is there by age.
    pub limited: bool,
    /// The names of the libraries there that they read.
    pub libraries: Vec<String>,
}

/// What was read from the other server.
#[derive(Debug, Serialize, ToSchema)]
pub struct Brought {
    /// Which kind of server it is, as it was asked for, and its address.
    pub from: String,
    pub address: String,
    pub version: String,
    /// The name the caller goes by there.
    pub their_name: String,
    /// Whether their key is of someone who manages that server. Its people
    /// are read only then.
    pub manages: bool,
    /// When it was read, in seconds since 1970.
    pub taken_at: i64,
    pub libraries: Vec<BroughtLibrary>,
    pub reading: BroughtReading,
    /// The reading lists and the collections they made or are shown.
    pub lists: i64,
    pub collections: i64,
    /// The series they want to read.
    pub wanted: i64,
    /// The pages they marked.
    pub pages: i64,
    pub people: Vec<BroughtPerson>,
}

impl From<binge_core::Brought> for Brought {
    fn from(brought: binge_core::Brought) -> Self {
        let reading = brought.reading;
        Self {
            from: brought.source,
            address: brought.address,
            version: brought.version,
            their_name: brought.their_name,
            manages: brought.manages,
            taken_at: brought.taken_at,
            libraries: brought.libraries.into_iter().map(Into::into).collect(),
            reading: BroughtReading {
                started: reading.started,
                finished: reading.finished,
                found: reading.found,
                timed: reading.timed,
                seconds: reading.seconds,
                dated: reading.dated,
                first_at: reading.first_at,
                last_at: reading.last_at,
            },
            lists: brought.lists,
            collections: brought.collections,
            wanted: brought.wanted,
            pages: brought.pages,
            people: brought
                .people
                .into_iter()
                .map(|person| BroughtPerson {
                    name: person.name,
                    manages: person.manages,
                    limited: person.limited,
                    libraries: person.libraries,
                })
                .collect(),
        }
    }
}

/// Where the caller stands with the server they are leaving.
#[derive(Debug, Serialize, ToSchema)]
pub struct BringStatus {
    /// Whether the other server is being read now.
    pub reading: bool,
    /// How far that is: the series whose files were asked for, of all.
    pub series_read: u32,
    pub series_total: u32,
    /// Why the last reading stopped, when it could not be done.
    pub problem: Option<String>,
    /// What the last reading that was done found. Left out while another
    /// is under way.
    pub brought: Option<Brought>,
}

async fn status_of(state: &AppState, owner: &Owner) -> Result<BringStatus, ApiError> {
    let reading = state.bringing.reading(owner.0.id);
    // Counting what was found is not done for an answer that only says
    // how far the next reading is.
    let brought = match state.catalog.brought(owner.0.id).await {
        _ if reading.under_way => None,
        Ok(brought) => Some(brought.into()),
        Err(CatalogError::NotFound) => None,
        Err(error) => return Err(error.into()),
    };
    Ok(BringStatus {
        reading: reading.under_way,
        series_read: reading.read.series,
        series_total: reading.read.of,
        problem: reading.problem,
        brought,
    })
}

/// Says where the caller stands with the server they are leaving: whether
/// it is being read, why the last reading stopped, and what the last one
/// that was done found. Only the owner can ask.
#[utoipa::path(
    get,
    path = "/bring",
    tag = "bring",
    operation_id = "getBringStatus",
    responses(
        (status = OK, body = BringStatus),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
    )
)]
async fn status(
    State(state): State<AppState>,
    owner: Owner,
) -> Result<Json<BringStatus>, ApiError> {
    Ok(Json(status_of(&state, &owner).await?))
}

/// Starts reading the server the caller is leaving, and answers at once.
/// `GET /bring` says how far it is, and in `problem` why it stopped when it
/// could not be done.
///
/// The other server is asked for its libraries and their files, what the
/// caller has read there and when, their reading lists, collections, want
/// to read and marked pages, and, with the key of someone who manages it,
/// who uses it. Nothing is changed there. What it said is kept in place of
/// what was read before, and nothing is made of it here yet. The key is not
/// kept. Only the owner can.
#[utoipa::path(
    post,
    path = "/bring",
    tag = "bring",
    operation_id = "startBringing",
    request_body = BringAsked,
    responses(
        (status = ACCEPTED, body = BringStatus),
        (status = BAD_REQUEST, description = "The address does not read", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = CONFLICT, description = "The other server is being read already", body = ApiError),
    )
)]
async fn start(
    State(state): State<AppState>,
    owner: Owner,
    Json(asked): Json<BringAsked>,
) -> Result<(StatusCode, Json<BringStatus>), ApiError> {
    state
        .bringing
        .begin(owner.0.id, asked.from, asked.address, asked.key)?;
    Ok((StatusCode::ACCEPTED, Json(status_of(&state, &owner).await?)))
}

/// Forgets what was read from the other server. What was already made of
/// it here stays. Only the owner can.
#[utoipa::path(
    delete,
    path = "/bring",
    tag = "bring",
    operation_id = "forgetBrought",
    responses(
        (status = NO_CONTENT, description = "It is forgotten"),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, description = "Nothing was read", body = ApiError),
    )
)]
async fn forget(State(state): State<AppState>, owner: Owner) -> Result<StatusCode, ApiError> {
    state.catalog.forget_brought(owner.0.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub struct LibraryHere {
    /// The library here that holds the same folder, or nothing when none
    /// does.
    pub library_id: Option<i64>,
}

/// Says which library here holds the folder of a library of the other
/// server, or that none does. Its files are then looked for in that
/// library, and what was said holds when the same server is read again.
/// Only the owner can.
///
/// Two servers rarely see a folder under the same path, so the one this
/// server took it for can be wrong.
#[utoipa::path(
    put,
    path = "/bring/libraries/{their_id}",
    tag = "bring",
    operation_id = "sayBroughtLibrary",
    params(("their_id" = i64, Path, description = "The library's id on the other server")),
    request_body = LibraryHere,
    responses(
        (status = OK, body = BringStatus),
        (status = BAD_REQUEST, description = "The library named is not on this server", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, description = "Nothing was read, or it has no such library", body = ApiError),
    )
)]
async fn say_library(
    State(state): State<AppState>,
    owner: Owner,
    Path(their_id): Path<i64>,
    Json(here): Json<LibraryHere>,
) -> Result<Json<BringStatus>, ApiError> {
    state
        .catalog
        .say_brought_library(owner.0.id, their_id, here.library_id)
        .await?;
    Ok(Json(status_of(&state, &owner).await?))
}
