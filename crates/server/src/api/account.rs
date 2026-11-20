//! What someone manages about their own account: their password, and the
//! tokens their apps use. Only for a caller who signed in with a password.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::{self, Caller};
use super::error::ApiError;
use super::session::Password;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(change_password))
        .routes(routes!(list_tokens, create_token))
        .routes(routes!(revoke_token))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PasswordChange {
    /// The password as it is now.
    #[schema(value_type = String, format = Password)]
    pub current: Password,
    /// The password to have from now on. At least 8 characters.
    #[schema(value_type = String, format = Password)]
    pub new: Password,
}

/// A token an app uses instead of a password. It can do what its user can,
/// except manage accounts.
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiToken {
    pub id: i64,
    /// What the person called it, such as the device it is on.
    pub name: String,
    /// When it was made, in seconds since 1970.
    pub created_at: i64,
    /// When it was last used, in seconds since 1970, to the minute. Missing
    /// when it never was.
    pub last_used_at: Option<i64>,
}

impl From<binge_core::ApiToken> for ApiToken {
    fn from(token: binge_core::ApiToken) -> Self {
        Self {
            id: token.id,
            name: token.name,
            created_at: token.created_at,
            last_used_at: token.last_used_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewToken {
    /// What to call it, such as the device it is for.
    pub name: String,
}

/// A token that was just made, with its secret.
// Without `Debug`, so the secret cannot end up in a log by being printed.
#[derive(Serialize, ToSchema)]
pub struct CreatedToken {
    pub token: ApiToken,
    /// What the app sends as `Authorization: Bearer <secret>`. The server
    /// does not keep it: this answer is the only time it is shown.
    pub secret: String,
}

/// Changes the caller's own password. Their sessions in other browsers end;
/// this one and their tokens stay.
#[utoipa::path(
    put,
    path = "/me/password",
    tag = "account",
    operation_id = "changePassword",
    request_body = PasswordChange,
    responses(
        (status = NO_CONTENT),
        (status = BAD_REQUEST, description = "The current password is wrong, or the new one will not do", body = ApiError),
        (status = FORBIDDEN, description = "The request came with a token", body = ApiError),
    )
)]
async fn change_password(
    State(state): State<AppState>,
    caller: Caller,
    headers: HeaderMap,
    Json(change): Json<PasswordChange>,
) -> Result<StatusCode, ApiError> {
    state
        .accounts
        .change_password(
            caller.user.id,
            change.current.as_str(),
            change.new.as_str(),
            auth::session_secret(&headers),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lists the caller's tokens, the newest first. Their secrets are not kept
/// and so not listed.
#[utoipa::path(
    get,
    path = "/tokens",
    tag = "account",
    operation_id = "listTokens",
    responses((status = OK, body = [ApiToken]), (status = FORBIDDEN, body = ApiError))
)]
async fn list_tokens(
    State(state): State<AppState>,
    caller: Caller,
) -> Result<Json<Vec<ApiToken>>, ApiError> {
    let tokens = state.accounts.tokens(caller.user.id).await?;
    Ok(Json(tokens.into_iter().map(ApiToken::from).collect()))
}

/// Makes a token for an app. Its secret is in this answer and nowhere else.
#[utoipa::path(
    post,
    path = "/tokens",
    tag = "account",
    operation_id = "createToken",
    request_body = NewToken,
    responses(
        (status = CREATED, body = CreatedToken),
        (status = BAD_REQUEST, description = "The name will not do", body = ApiError),
        (status = FORBIDDEN, body = ApiError),
    )
)]
async fn create_token(
    State(state): State<AppState>,
    caller: Caller,
    Json(new): Json<NewToken>,
) -> Result<(StatusCode, Json<CreatedToken>), ApiError> {
    let (token, secret) = state
        .accounts
        .create_token(caller.user.id, &new.name)
        .await?;
    let created = CreatedToken {
        token: token.into(),
        secret,
    };
    Ok((StatusCode::CREATED, Json(created)))
}

/// Revokes one of the caller's tokens. It stops working at once.
#[utoipa::path(
    delete,
    path = "/tokens/{id}",
    tag = "account",
    operation_id = "revokeToken",
    params(("id" = i64, Path, description = "The token")),
    responses(
        (status = NO_CONTENT),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn revoke_token(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.accounts.revoke_token(caller.user.id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
