//! Getting in: first-run setup, signing in and out, and who is signed in.

use std::fmt;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::{self, Caller};
use super::error::ApiError;
use crate::app::AppState;

/// The routes anyone may call: nobody is signed in before these.
pub(super) fn open_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(set_up))
        .routes(routes!(sign_in))
}

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(me))
        .routes(routes!(names))
        .routes(routes!(sign_out))
}

/// A password as a request brings it. It does not show itself when a
/// request is printed, so it cannot end up in a log that way.
#[derive(Deserialize)]
#[serde(transparent)]
pub struct Password(String);

impl Password {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[hidden]")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Reads every library and manages the server: libraries and users.
    Owner,
    /// Reads the libraries the owner gave them.
    Member,
}

/// A person who may use the server.
#[derive(Debug, Serialize, ToSchema)]
pub struct User {
    pub id: i64,
    /// What the person signs in with.
    pub name: String,
    pub role: Role,
    /// Ids of the libraries the person may read: every library for the
    /// owner, the ones the owner gave them for a member.
    pub libraries: Vec<i64>,
    /// When the account was made, in seconds since 1970.
    pub created_at: i64,
}

impl From<binge_core::Account> for User {
    fn from(account: binge_core::Account) -> Self {
        let user = account.user;
        Self {
            id: user.id.0,
            name: user.name,
            role: if user.is_owner {
                Role::Owner
            } else {
                Role::Member
            },
            libraries: account.libraries,
            created_at: user.created_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct Credentials {
    /// The name to sign in with. Upper and lower case are the same.
    pub name: String,
    #[schema(value_type = String, format = Password)]
    pub password: Password,
}

type WithCookie<T> = ([(header::HeaderName, HeaderValue); 1], T);

/// Answers with the user, and gives the browser its session.
async fn signed_in(
    state: &AppState,
    (user, secret): (binge_core::User, String),
) -> Result<WithCookie<Json<User>>, ApiError> {
    let account = state.accounts.account(user.id).await?;
    let cookie = auth::session_cookie(&secret)?;
    Ok(([(header::SET_COOKIE, cookie)], Json(account.into())))
}

/// First-run setup: makes the owner of a server that has none, and signs
/// them in. `needs_setup` of `/health` says whether this is still to do.
///
/// What was read before the server had accounts becomes the owner's.
#[utoipa::path(
    post,
    path = "/setup",
    tag = "account",
    operation_id = "setUp",
    request_body = Credentials,
    responses(
        (status = CREATED, description = "The owner, signed in: the session cookie is set", body = User),
        (status = BAD_REQUEST, description = "The name or the password will not do", body = ApiError),
        (status = CONFLICT, description = "The server already has an owner", body = ApiError),
    )
)]
async fn set_up(
    State(state): State<AppState>,
    Json(new): Json<Credentials>,
) -> Result<(StatusCode, WithCookie<Json<User>>), ApiError> {
    let owner = state
        .accounts
        .create_owner(&new.name, new.password.as_str())
        .await?;
    Ok((StatusCode::CREATED, signed_in(&state, owner).await?))
}

/// Signs in with a name and password. The answer sets the session cookie,
/// which is what later requests from the same browser are known by.
///
/// An app signs in once this way, makes itself a token with `POST /tokens`,
/// signs out, and uses the token from then on.
#[utoipa::path(
    post,
    path = "/sign-in",
    tag = "account",
    operation_id = "signIn",
    request_body = Credentials,
    responses(
        (status = OK, description = "Signed in: the session cookie is set", body = User),
        (status = UNAUTHORIZED, description = "The name and password do not match", body = ApiError),
        (status = TOO_MANY_REQUESTS, description = "Too many attempts at once", body = ApiError),
    )
)]
async fn sign_in(
    State(state): State<AppState>,
    Json(shown): Json<Credentials>,
) -> Result<WithCookie<Json<User>>, ApiError> {
    let user = state
        .accounts
        .sign_in(&shown.name, shown.password.as_str())
        .await?;
    signed_in(&state, user).await
}

/// Returns who makes the request. This is also how an app finds out whether
/// its token still works.
#[utoipa::path(
    get,
    path = "/me",
    tag = "account",
    operation_id = "getMe",
    responses((status = OK, body = User), (status = UNAUTHORIZED, body = ApiError))
)]
async fn me(State(state): State<AppState>, caller: Caller) -> Result<Json<User>, ApiError> {
    Ok(Json(state.accounts.account(caller.user.id).await?.into()))
}

/// A person of the server as the others know them: by name.
#[derive(Debug, Serialize, ToSchema)]
pub struct UserName {
    pub id: i64,
    pub name: String,
}

impl From<binge_core::UserName> for UserName {
    fn from(user: binge_core::UserName) -> Self {
        Self {
            id: user.id.0,
            name: user.name,
        }
    }
}

/// Lists everyone on the server by name, the caller too, in name order:
/// what a person needs to know of the others to share something with
/// them. It says nothing else about them.
#[utoipa::path(
    get,
    path = "/users/names",
    tag = "account",
    operation_id = "listUserNames",
    responses((status = OK, body = [UserName]))
)]
async fn names(State(state): State<AppState>, _: Caller) -> Result<Json<Vec<UserName>>, ApiError> {
    let names = state.accounts.names().await?;
    Ok(Json(names.into_iter().map(UserName::from).collect()))
}

/// Ends the session the request came with, and takes its cookie back. A
/// token is not affected: it is revoked with `DELETE /tokens/{id}`.
#[utoipa::path(
    post,
    path = "/sign-out",
    tag = "account",
    operation_id = "signOut",
    responses((status = NO_CONTENT), (status = UNAUTHORIZED, body = ApiError))
)]
async fn sign_out(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<WithCookie<StatusCode>, ApiError> {
    if let Some(secret) = auth::session_secret(&headers) {
        state.accounts.sign_out(secret).await?;
    }
    Ok((
        [(header::SET_COOKIE, auth::no_session_cookie()?)],
        StatusCode::NO_CONTENT,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_does_not_print_itself() {
        let shown: Credentials =
            serde_json::from_str(r#"{ "name": "Ana", "password": "correct horse" }"#).unwrap();
        assert_eq!(shown.password.as_str(), "correct horse");
        let printed = format!("{shown:?}");
        assert!(printed.contains("Ana"));
        assert!(!printed.contains("correct horse"), "{printed}");
    }
}
