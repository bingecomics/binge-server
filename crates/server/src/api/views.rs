//! Views: a library narrowed and ordered, kept under a name, to be a row
//! on the home page or an entry in the menu.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::{Caller, Owner};
use super::error::ApiError;
use super::libraries::Asked;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, add))
        .routes(routes!(change, delete))
}

/// A library narrowed and ordered, kept under a name. Its series are what
/// `GET /libraries/{library_id}/series` answers when asked for `asked`.
#[derive(Debug, Serialize, ToSchema)]
pub struct View {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub asked: Asked,
    /// Whether it is a row on the home page.
    pub on_home: bool,
    /// Whether it is an entry in the menu.
    pub in_menu: bool,
}

impl From<binge_core::View> for View {
    fn from(view: binge_core::View) -> Self {
        Self {
            id: view.id,
            library_id: view.library_id,
            name: view.name,
            asked: Asked::of(view.order, view.narrowing),
            on_home: view.on_home,
            in_menu: view.in_menu,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewView {
    pub library_id: i64,
    pub name: String,
    /// The whole library by name when left out.
    #[serde(default)]
    pub asked: Asked,
    #[serde(default)]
    pub on_home: bool,
    #[serde(default)]
    pub in_menu: bool,
}

/// What to change of a view. What is left out stays as it is.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ViewChange {
    pub name: Option<String>,
    /// Replaces the order and everything the view narrows by.
    pub asked: Option<Asked>,
    pub on_home: Option<bool>,
    pub in_menu: Option<bool>,
}

/// Lists the views of the libraries the caller may read, in name order.
#[utoipa::path(
    get,
    path = "/views",
    tag = "views",
    operation_id = "listViews",
    responses((status = OK, body = [View]))
)]
async fn list(State(state): State<AppState>, caller: Caller) -> Result<Json<Vec<View>>, ApiError> {
    let views = state.catalog.views(caller.user.id).await?;
    Ok(Json(views.into_iter().map(View::from).collect()))
}

/// Keeps a view of a library. Everyone who may read the library sees it.
/// Only the owner can.
#[utoipa::path(
    post,
    path = "/views",
    tag = "views",
    operation_id = "addView",
    request_body = NewView,
    responses(
        (status = CREATED, body = View),
        (status = BAD_REQUEST, description = "It has no name, a year is not written as one, or there are too many views or values", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, description = "There is no such library", body = ApiError),
    )
)]
async fn add(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Json(new): Json<NewView>,
) -> Result<(StatusCode, Json<View>), ApiError> {
    let order = new.asked.order();
    let new = binge_core::NewView {
        library_id: new.library_id,
        name: new.name,
        order,
        narrowing: new.asked.narrowing()?,
        on_home: new.on_home,
        in_menu: new.in_menu,
    };
    let view = state.catalog.add_view(owner.id, new).await?;
    Ok((StatusCode::CREATED, Json(view.into())))
}

/// Changes a view. Only the owner can.
#[utoipa::path(
    patch,
    path = "/views/{id}",
    tag = "views",
    operation_id = "changeView",
    params(("id" = i64, Path, description = "The view")),
    request_body = ViewChange,
    responses(
        (status = OK, body = View),
        (status = BAD_REQUEST, body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn change(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(change): Json<ViewChange>,
) -> Result<Json<View>, ApiError> {
    let asked = match change.asked {
        Some(asked) => Some((asked.order(), asked.narrowing()?)),
        None => None,
    };
    let change = binge_core::ViewChange {
        name: change.name,
        asked,
        on_home: change.on_home,
        in_menu: change.in_menu,
    };
    let view = state.catalog.change_view(owner.id, id, change).await?;
    Ok(Json(view.into()))
}

/// Deletes a view. The library and its books are not touched. Only the
/// owner can.
#[utoipa::path(
    delete,
    path = "/views/{id}",
    tag = "views",
    operation_id = "deleteView",
    params(("id" = i64, Path, description = "The view")),
    responses(
        (status = NO_CONTENT),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn delete(
    State(state): State<AppState>,
    _: Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.catalog.delete_view(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
