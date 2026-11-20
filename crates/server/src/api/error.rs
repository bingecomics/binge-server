//! The one shape every API error has.

use std::fmt;

use axum::Json;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use binge_core::{AccountError, BackupError, CatalogError};
use serde::Serialize;
use utoipa::ToSchema;

use crate::update::UpdateError;

/// What went wrong, as an identifier clients can branch on.
#[derive(Clone, Copy, Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    /// The request does not show who makes it, or what it shows is no longer
    /// good. Signing in again is what helps.
    Unauthorized,
    /// The name and password given to sign in do not match.
    InvalidCredentials,
    /// Who makes the request is known and may not do this.
    Forbidden,
    NotFound,
    MethodNotAllowed,
    HostNotAllowed,
    /// The request is fine but clashes with what is already there.
    Conflict,
    /// Too much at once. The same request may work in a moment.
    TooManyRequests,
    Internal,
}

impl ErrorCode {
    fn status(self) -> StatusCode {
        match self {
            Self::InvalidRequest => StatusCode::BAD_REQUEST,
            Self::Unauthorized | Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::HostNotAllowed => StatusCode::FORBIDDEN,
            Self::Conflict => StatusCode::CONFLICT,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The closest code for an error response axum produced on its own.
    fn for_status(status: StatusCode) -> Self {
        match status {
            StatusCode::NOT_FOUND => Self::NotFound,
            StatusCode::METHOD_NOT_ALLOWED => Self::MethodNotAllowed,
            status if status.is_server_error() => Self::Internal,
            _ => Self::InvalidRequest,
        }
    }
}

/// Body of every error response the API sends.
///
/// Handlers return it directly. Errors that axum produces on its own (a wrong
/// method, a body that does not parse) are rewritten into it by
/// [`json_errors`], so clients only ever have one shape to decode.
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiError {
    pub code: ErrorCode,
    /// Explanation for people.
    pub message: String,
    /// The id of the series, group, reading list or person that is in the
    /// way, when the request clashes with one: the series, the list or the
    /// person that has the name, the group that is the same.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflicts_with: Option<i64>,
}

/// Marks a response as already carrying an [`ApiError`] body.
#[derive(Clone, Copy)]
struct IsApiError;

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            conflicts_with: None,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    /// The request does not show who makes it. `message` says what to do.
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    /// Who makes the request may not do this. `message` says why.
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }

    /// Something is wrong inside the server. The caller has logged what.
    pub fn internal() -> Self {
        Self::new(ErrorCode::Internal, binge_core::FAILED_FOR_PEOPLE)
    }

    /// The answer to an error from the core. `code` is what kind of error
    /// it is to a client; without one it is the server's own failure, which
    /// goes to the log. `for_people` is the core's sentence about it.
    fn of_core(code: Option<ErrorCode>, error: &dyn fmt::Display, for_people: &str) -> Self {
        let code = code.unwrap_or_else(|| {
            tracing::error!(%error, "request failed");
            ErrorCode::Internal
        });
        Self::new(code, for_people)
    }

    /// The request named a host this server does not answer to.
    pub fn host_not_allowed(host: &str) -> Self {
        let message = if host.is_empty() {
            "The request's Host header is not valid.".to_owned()
        } else {
            format!(
                "This server does not answer to the name \"{host}\". Open it by IP address, \
                 or start it with --allowed-host {host}."
            )
        };
        Self::new(ErrorCode::HostNotAllowed, message)
    }

    fn with_status(self, status: StatusCode) -> Response {
        let mut response = (status, Json(self)).into_response();
        if status == StatusCode::UNAUTHORIZED {
            // What HTTP asks of a 401. It names the scheme apps use; a
            // browser does not ask for a password because of it.
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response.extensions_mut().insert(IsApiError);
        response
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.code.status();
        self.with_status(status)
    }
}

/// Longest error text taken over from a response axum produced.
const MAX_DETAIL: usize = 16 * 1024;

/// Gives every error response an [`ApiError`] body, keeping its status and
/// headers such as `Allow`.
pub(super) async fn json_errors(response: Response) -> Response {
    let status = response.status();
    let is_error = status.is_client_error() || status.is_server_error();
    if !is_error || response.extensions().get::<IsApiError>().is_some() {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    // What a client sent wrong is worth passing on; what went wrong inside
    // the server is not.
    let detail = if status.is_client_error() {
        axum::body::to_bytes(body, MAX_DETAIL)
            .await
            .ok()
            .and_then(|bytes| String::from_utf8(bytes.into()).ok())
            .filter(|text| !text.trim().is_empty())
    } else {
        None
    };
    let message = detail
        .unwrap_or_else(|| format!("{}.", status.canonical_reason().unwrap_or("Request failed")));

    let mut rewritten = ApiError::new(ErrorCode::for_status(status), message).with_status(status);
    parts.headers.remove(header::CONTENT_TYPE);
    parts.headers.remove(header::CONTENT_LENGTH);
    rewritten.headers_mut().extend(parts.headers);
    rewritten
}

/// The file a request brought as its body, unless it is more than the
/// server takes, which `too_big` says, or did not come whole. `what` names
/// it in a sentence: "picture", "file".
pub(super) fn whole(
    body: Result<Bytes, BytesRejection>,
    too_big: fn() -> CatalogError,
    what: &str,
) -> Result<Bytes, ApiError> {
    body.map_err(|refused| {
        if refused.status() == StatusCode::PAYLOAD_TOO_LARGE {
            too_big().into()
        } else {
            ApiError::new(
                ErrorCode::InvalidRequest,
                format!("The {what} did not arrive whole. Try again."),
            )
        }
    })
}

impl From<CatalogError> for ApiError {
    fn from(error: CatalogError) -> Self {
        let code = match &error {
            // A missing file is logged where it is noticed.
            CatalogError::NotFound
            | CatalogError::NoSuchPage
            | CatalogError::NoSuchRule
            | CatalogError::NoSuchPerson
            | CatalogError::FileMissing(_) => Some(ErrorCode::NotFound),
            CatalogError::InvalidPath(_) | CatalogError::InvalidPage | CatalogError::Invalid(_) => {
                Some(ErrorCode::InvalidRequest)
            }
            CatalogError::AlreadyAdded
            | CatalogError::FileChanged(_)
            | CatalogError::NameTaken(_)
            | CatalogError::GroupExists(_)
            | CatalogError::ListNameTaken(_)
            | CatalogError::NameOfAnother(_)
            | CatalogError::NotEmpty => Some(ErrorCode::Conflict),
            _ => None,
        };
        Self {
            conflicts_with: error.in_the_way(),
            ..Self::of_core(code, &error, error.for_people())
        }
    }
}

impl From<AccountError> for ApiError {
    fn from(error: AccountError) -> Self {
        let code = match &error {
            AccountError::WrongCredentials => Some(ErrorCode::InvalidCredentials),
            AccountError::Busy => Some(ErrorCode::TooManyRequests),
            AccountError::AlreadySetUp | AccountError::NameTaken => Some(ErrorCode::Conflict),
            AccountError::NoSuchUser | AccountError::NoSuchToken => Some(ErrorCode::NotFound),
            AccountError::WrongPassword
            | AccountError::Invalid(_)
            | AccountError::NoSuchLibrary
            | AccountError::IsOwner => Some(ErrorCode::InvalidRequest),
            _ => None,
        };
        Self::of_core(code, &error, error.for_people())
    }
}

impl From<BackupError> for ApiError {
    fn from(error: BackupError) -> Self {
        // Nothing the caller sent decides whether a copy can be written.
        Self::of_core(None, &error, error.for_people())
    }
}

impl From<UpdateError> for ApiError {
    fn from(error: UpdateError) -> Self {
        let code = match &error {
            // The request is fine; where the server stands is in its way.
            UpdateError::Cannot(_) | UpdateError::Busy | UpdateError::NotTheNewest => {
                Some(ErrorCode::Conflict)
            }
            _ => None,
        };
        Self::of_core(code, &error, error.for_people())
    }
}
