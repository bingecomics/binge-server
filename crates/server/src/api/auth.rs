//! The check that a request is allowed: who makes it, and how they showed it.
//!
//! A browser shows a session cookie, which it got by signing in with a name
//! and password. An app shows an API token in `Authorization: Bearer`. The
//! check runs as a layer around every route but the few that must be open,
//! so a route cannot be left unguarded by forgetting to ask who is calling.

use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, Uri, header};
use axum::middleware::Next;
use axum::response::Response;
use binge_core::{Accounts, User};

use super::error::ApiError;

/// Name of the cookie that holds a session's secret.
pub const SESSION_COOKIE: &str = "binge_session";

/// How long a browser keeps the cookie, in seconds: about 400 days, the
/// longest browsers allow. The server ends a session much sooner when it is
/// not used; the cookie only must not end before the session does.
const COOKIE_MAX_AGE: u32 = 400 * 24 * 60 * 60;

/// How the caller showed who they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proof {
    /// A session cookie: they signed in with their password.
    Session,
    /// An API token.
    Token,
}

/// Who makes the request. Handlers take it as an argument; it is there for
/// every route inside the check.
#[derive(Clone, Debug)]
pub struct Caller {
    pub user: User,
    pub proof: Proof,
}

/// The caller, when they are the server's owner. Anyone else is refused.
#[derive(Clone, Debug)]
pub struct Owner(pub User);

impl<S: Send + Sync> FromRequestParts<S> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        parts.extensions.get::<Caller>().cloned().ok_or_else(|| {
            // A mistake in how the routes are put together, not in the
            // request. Refusing is the safe answer to it.
            tracing::error!(path = %parts.uri.path(), "a route asks who is calling outside the check");
            ApiError::internal()
        })
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Owner {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        let caller = Caller::from_request_parts(parts, state).await?;
        if !caller.user.is_owner {
            return Err(ApiError::forbidden(
                "Only the owner of this server can do that.",
            ));
        }
        Ok(Self(caller.user))
    }
}

/// Layer that lets a request through only when it shows who makes it.
pub async fn signed_in(
    State(accounts): State<Accounts>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let caller = identify(&accounts, request.headers()).await?;
    request.extensions_mut().insert(caller);
    Ok(next.run(request).await)
}

/// Layer, inside [`signed_in`], for what an API token may not do: change the
/// account it belongs to, or other accounts. A token that leaks can then be
/// revoked by its owner, and cannot have made itself a successor.
pub async fn in_person(caller: Caller, request: Request, next: Next) -> Result<Response, ApiError> {
    if caller.proof != Proof::Session {
        return Err(ApiError::forbidden(
            "An app token cannot manage accounts. Sign in with your name and password.",
        ));
    }
    Ok(next.run(request).await)
}

async fn identify(accounts: &Accounts, headers: &HeaderMap) -> Result<Caller, ApiError> {
    // A request that brings a token is judged by the token alone, also when
    // a cookie came along with it.
    if let Some(token) = bearer_token(headers) {
        let user = accounts.token_user(token).await?.ok_or_else(|| {
            ApiError::unauthorized("That token is not valid. It may have been revoked.")
        })?;
        return Ok(Caller {
            user,
            proof: Proof::Token,
        });
    }
    let Some(secret) = session_secret(headers) else {
        return Err(ApiError::unauthorized("Sign in to use this server."));
    };
    let user = accounts
        .session_user(secret)
        .await?
        .ok_or_else(|| ApiError::unauthorized("Your session has ended. Sign in again."))?;
    Ok(Caller {
        user,
        proof: Proof::Session,
    })
}

/// The token of the request's `Authorization: Bearer <token>` header, if it
/// has one. It may be empty, which no token is.
///
/// Only `Bearer` is this server's to judge. Another scheme, such as the
/// `Basic` that a proxy in front of the server asks browsers for, is as if
/// the header were not there: the request then stands on its cookie, and is
/// held to what holds for cookies.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let shown = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = shown.split_once(' ').unwrap_or((shown, ""));
    scheme
        .eq_ignore_ascii_case("bearer")
        .then_some(token.trim())
}

/// The secret in the request's session cookie, if it brings one.
pub fn session_secret(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|cookie| cookie.trim().split_once('='))
        .find_map(|(name, value)| (name == SESSION_COOKIE && !value.is_empty()).then_some(value))
}

/// The `Set-Cookie` value that gives a browser its session.
///
/// `HttpOnly` keeps the secret from scripts. `SameSite=Lax` keeps other
/// sites from using it. It is not marked `Secure`, because the server speaks
/// plain HTTP on a home network; see ADR 0010.
pub fn session_cookie(secret: &str) -> Result<HeaderValue, ApiError> {
    cookie(secret, COOKIE_MAX_AGE)
}

/// The `Set-Cookie` value that takes the session away from a browser.
pub fn no_session_cookie() -> Result<HeaderValue, ApiError> {
    cookie("", 0)
}

fn cookie(secret: &str, max_age: u32) -> Result<HeaderValue, ApiError> {
    let cookie =
        format!("{SESSION_COOKIE}={secret}; Path=/api; HttpOnly; SameSite=Lax; Max-Age={max_age}");
    let mut value = HeaderValue::from_str(&cookie).map_err(|_| {
        tracing::error!("a session secret does not fit in a cookie");
        ApiError::internal()
    })?;
    // Keeps it out of anything that prints headers.
    value.set_sensitive(true);
    Ok(value)
}

/// Layer that refuses a change asked for by a page of another origin.
///
/// A browser sends the session cookie with requests other pages make, and
/// `SameSite` does not stop a page on another port of the same machine. So a
/// request that changes something and relies on the cookie must come from
/// the server's own pages. Requests that bring a token are left alone: no
/// page can add one without the browser asking the server first.
pub async fn same_origin(request: Request, next: Next) -> Result<Response, ApiError> {
    let reads = matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    let brings_token = bearer_token(request.headers()).is_some();
    if !reads && !brings_token && from_elsewhere(&request) {
        return Err(ApiError::forbidden(
            "This request came from a page that is not this server's.",
        ));
    }
    Ok(next.run(request).await)
}

/// Whether a browser says the request was made by a page of another origin.
/// A client that is not a browser says nothing, and has no cookie to misuse.
fn from_elsewhere(request: &Request) -> bool {
    let headers = request.headers();
    // The browser's own word for it, which a proxy in between cannot blur.
    if let Some(site) = headers.get("sec-fetch-site") {
        return site != "same-origin" && site != "none";
    }
    let Some(origin) = headers.get(header::ORIGIN) else {
        return false;
    };
    let origin = origin
        .to_str()
        .ok()
        .and_then(|origin| origin.parse::<Uri>().ok());
    let asked = request
        .uri()
        .authority()
        .map(|host| host.as_str())
        .or_else(|| {
            headers
                .get(header::HOST)
                .and_then(|host| host.to_str().ok())
        });
    match (origin.as_ref().and_then(Uri::authority), asked) {
        (Some(origin), Some(asked)) => !origin.as_str().eq_ignore_ascii_case(asked),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(*name, HeaderValue::from_static(value));
        }
        headers
    }

    #[test]
    fn the_session_is_found_among_other_cookies() {
        let secret = |pairs| session_secret(&headers(pairs)).map(str::to_owned);
        assert_eq!(
            secret(&[("cookie", "binge_session=abc")]).as_deref(),
            Some("abc")
        );
        assert_eq!(
            secret(&[("cookie", "theme=dark; binge_session=abc; other=1")]).as_deref(),
            Some("abc")
        );
        assert_eq!(
            secret(&[("cookie", "theme=dark"), ("cookie", "binge_session=abc")]).as_deref(),
            Some("abc")
        );
        assert_eq!(secret(&[("cookie", "binge_session=")]), None);
        assert_eq!(secret(&[("cookie", "not_binge_session=abc")]), None);
        assert_eq!(secret(&[]), None);
    }

    #[test]
    fn a_token_comes_as_a_bearer() {
        let token = |shown: &'static str| {
            bearer_token(&headers(&[("authorization", shown)])).map(str::to_owned)
        };
        assert_eq!(token("Bearer binge_abc").as_deref(), Some("binge_abc"));
        assert_eq!(token("bearer  binge_abc ").as_deref(), Some("binge_abc"));
        // A token that is not there is still a token shown, and a bad one.
        assert_eq!(token("Bearer ").as_deref(), Some(""));
        assert_eq!(token("Bearer").as_deref(), Some(""));
        // Not this server's kind of proof.
        assert_eq!(token("Basic made-up"), None);
        assert_eq!(token("binge_abc"), None);
        assert_eq!(bearer_token(&headers(&[])), None);
    }

    #[test]
    fn the_cookie_is_kept_from_scripts_and_other_sites() {
        let set = session_cookie("abc").unwrap();
        assert!(set.is_sensitive());
        let set = set.to_str().unwrap();
        assert!(set.starts_with("binge_session=abc; "));
        for part in ["HttpOnly", "SameSite=Lax", "Path=/api"] {
            assert!(set.contains(part), "{part}");
        }
        let unset = no_session_cookie().unwrap();
        assert!(unset.to_str().unwrap().contains("Max-Age=0"));
    }

    #[test]
    fn a_page_of_another_origin_is_told_apart() {
        let elsewhere = |pairs| {
            let mut request = Request::new(axum::body::Body::empty());
            *request.headers_mut() = headers(pairs);
            from_elsewhere(&request)
        };
        // Not a browser.
        assert!(!elsewhere(&[("host", "nas.local:8686")]));
        // The server's own page.
        assert!(!elsewhere(&[
            ("host", "nas.local:8686"),
            ("origin", "http://NAS.local:8686")
        ]));
        assert!(!elsewhere(&[
            ("host", "127.0.0.1:8686"),
            ("origin", "https://comics.example"),
            ("sec-fetch-site", "same-origin")
        ]));
        // Another site, another port of this machine, and a page that hides
        // where it is from.
        assert!(elsewhere(&[
            ("host", "nas.local:8686"),
            ("origin", "http://evil.example")
        ]));
        assert!(elsewhere(&[
            ("host", "nas.local:8686"),
            ("origin", "http://nas.local:9000")
        ]));
        assert!(elsewhere(&[("host", "nas.local:8686"), ("origin", "null")]));
        assert!(elsewhere(&[
            ("host", "nas.local:8686"),
            ("origin", "http://nas.local:8686"),
            ("sec-fetch-site", "same-site")
        ]));
        assert!(elsewhere(&[
            ("host", "nas.local:8686"),
            ("sec-fetch-site", "cross-site")
        ]));
    }
}
