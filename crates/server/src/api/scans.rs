//! What the scans changed, to look back on, and setting aside the files
//! that could not be read.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Owner;
use super::browse::{Book, PartQuery};
use super::error::ApiError;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(changes))
        .routes(routes!(set_aside, take_back))
}

/// A scan that changed what a library holds, with how many files of each
/// kind it found changed.
#[derive(Debug, Serialize, ToSchema)]
pub struct ScanRecord {
    pub id: i64,
    pub library_id: i64,
    /// When it started, in seconds since 1970.
    pub started_at: i64,
    /// When it had gone through the folder.
    pub ended_at: i64,
    /// Books the library never held.
    pub arrived: i64,
    /// Books the library held before, gone since, and here again.
    pub back: i64,
    /// Books whose file is at another path.
    pub moved: i64,
    /// Books whose file is no longer there.
    pub gone: i64,
}

impl From<binge_core::ScanRecord> for ScanRecord {
    fn from(scan: binge_core::ScanRecord) -> Self {
        Self {
            id: scan.id,
            library_id: scan.library_id,
            started_at: scan.started_at,
            ended_at: scan.ended_at,
            arrived: scan.arrived,
            back: scan.back,
            moved: scan.moved,
            gone: scan.gone,
        }
    }
}

/// How a scan found a file changed.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// A book the library never held. A second file of a book it holds is
    /// one too.
    Arrived,
    /// A book the library held before, gone since, and here again.
    Back,
    /// A book whose file is at another path.
    Moved,
    /// A book whose file is no longer there.
    Gone,
}

impl From<ChangeKind> for binge_core::ChangeKind {
    fn from(kind: ChangeKind) -> Self {
        match kind {
            ChangeKind::Arrived => Self::Arrived,
            ChangeKind::Back => Self::Back,
            ChangeKind::Moved => Self::Moved,
            ChangeKind::Gone => Self::Gone,
        }
    }
}

impl From<binge_core::ChangeKind> for ChangeKind {
    fn from(kind: binge_core::ChangeKind) -> Self {
        match kind {
            binge_core::ChangeKind::Arrived => Self::Arrived,
            binge_core::ChangeKind::Back => Self::Back,
            binge_core::ChangeKind::Moved => Self::Moved,
            binge_core::ChangeKind::Gone => Self::Gone,
        }
    }
}

/// One file a scan found changed.
#[derive(Debug, Serialize, ToSchema)]
pub struct ScanChange {
    pub id: i64,
    pub kind: ChangeKind,
    /// Where the file is, from the library's folder, or where it was for
    /// one that is gone.
    pub path: String,
    /// Where a moved file was before.
    pub from_path: Option<String>,
    /// The book as the library holds it now. Missing when it holds it no
    /// longer, and for a file that could not be read.
    pub book: Option<Book>,
}

/// A part of what one scan changed.
#[derive(Debug, Serialize, ToSchema)]
pub struct ScanChanges {
    pub scan: ScanRecord,
    pub changes: Vec<ScanChange>,
    /// How many changes the whole list holds.
    pub total: i64,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ChangesQuery {
    /// Lists the changes of one kind only.
    #[param(inline)]
    pub kind: Option<ChangeKind>,
}

/// Lists the scans that changed what a library holds, the latest first.
/// A scan that found the library as it was is not listed, and only the
/// latest fifty are kept. Only the owner can ask.
#[utoipa::path(
    get,
    path = "/libraries/{id}/scans",
    tag = "libraries",
    operation_id = "listScans",
    params(("id" = i64, Path, description = "The library")),
    responses(
        (status = OK, body = [ScanRecord]),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn list(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
) -> Result<Json<Vec<ScanRecord>>, ApiError> {
    let scans = state.catalog.scans_of(owner.id, id).await?;
    Ok(Json(scans.into_iter().map(ScanRecord::from).collect()))
}

/// Lists a part of what one scan changed: the books that arrived first,
/// then the ones that came back, moved and went, each by its paths. Only
/// the owner can ask.
#[utoipa::path(
    get,
    path = "/scans/{id}/changes",
    tag = "libraries",
    operation_id = "listScanChanges",
    params(("id" = i64, Path, description = "The scan"), ChangesQuery, PartQuery),
    responses(
        (status = OK, body = ScanChanges),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn changes(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Query(query): Query<ChangesQuery>,
    Query(part): Query<PartQuery>,
) -> Result<Json<ScanChanges>, ApiError> {
    let kind = query.kind.map(Into::into);
    let found = state.catalog.scan_changes(owner.id, id, kind, part.span());
    let (scan, part) = found.await?;
    Ok(Json(ScanChanges {
        scan: scan.into(),
        changes: part
            .items
            .into_iter()
            .map(|change| ScanChange {
                id: change.id,
                kind: change.kind.into(),
                path: change.path,
                from_path: change.from_path,
                book: change.book.map(Into::into),
            })
            .collect(),
        total: part.total,
    }))
}

/// Sets a file that could not be read aside: seen, and no longer listed
/// with the others or counted. The file is not touched. A scan that reads
/// it again and still cannot lists it again. Only the owner can.
#[utoipa::path(
    put,
    path = "/books/{id}/set-aside",
    tag = "libraries",
    operation_id = "setFileAside",
    params(("id" = i64, Path, description = "The file, as `GET /libraries/{id}/unreadable` lists it")),
    responses(
        (status = NO_CONTENT),
        (status = BAD_REQUEST, description = "The book can be read", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn set_aside(
    State(state): State<AppState>,
    _: Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.catalog.set_aside(id, true).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Takes a file back from the ones set aside. Only the owner can.
#[utoipa::path(
    delete,
    path = "/books/{id}/set-aside",
    tag = "libraries",
    operation_id = "takeFileBack",
    params(("id" = i64, Path, description = "The file")),
    responses(
        (status = NO_CONTENT),
        (status = BAD_REQUEST, description = "The book can be read", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn take_back(
    State(state): State<AppState>,
    _: Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.catalog.set_aside(id, false).await?;
    Ok(StatusCode::NO_CONTENT)
}
