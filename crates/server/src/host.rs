//! Refuses requests addressed to a name this server does not answer to.
//!
//! A web page cannot normally read a server on its visitor's machine or home
//! network. It can if it reaches that server under the page's own host name,
//! by pointing the name at the server's address (DNS rebinding). Every such
//! request names the attacker's host, so the server only answers to names it
//! knows: IP addresses, `localhost`, and the names the owner allowed.

use std::net::IpAddr;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header;
use axum::http::uri::Authority;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::api::ApiError;

/// Host names the owner allowed on top of the ones that always work.
#[derive(Clone, Debug, Default)]
pub struct AllowedHosts(Arc<[String]>);

impl AllowedHosts {
    pub fn new(names: impl IntoIterator<Item = String>) -> Self {
        Self(names.into_iter().map(|name| normalize(&name)).collect())
    }

    /// Whether a request naming `host` (without a port) may be answered.
    pub fn allows(&self, host: &str) -> bool {
        let host = normalize(host);
        let address = host.trim_start_matches('[').trim_end_matches(']');
        // A browser sends an address only when the page was opened by that
        // address, which a page on another host cannot arrange.
        address.parse::<IpAddr>().is_ok()
            || host == "localhost"
            || host.ends_with(".localhost")
            || self.0.contains(&host)
    }
}

fn normalize(name: &str) -> String {
    name.trim().trim_end_matches('.').to_ascii_lowercase()
}

/// Middleware that answers 403 when the request names a host not allowed.
pub async fn check(State(allowed): State<AllowedHosts>, request: Request, next: Next) -> Response {
    match requested_host(&request) {
        Some(host) if !allowed.allows(&host) => ApiError::host_not_allowed(&host).into_response(),
        _ => next.run(request).await,
    }
}

/// The host the request names, without the port. `None` when it names none,
/// which no browser does; an empty name when the header cannot be read.
fn requested_host(request: &Request) -> Option<String> {
    if let Some(host) = request.uri().host() {
        return Some(host.to_owned());
    }
    let value = request.headers().get(header::HOST)?;
    let host = value
        .to_str()
        .ok()
        .and_then(|value| value.parse::<Authority>().ok())
        .map(|authority| authority.host().to_owned());
    Some(host.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_and_localhost_always_work() {
        let allowed = AllowedHosts::default();
        for host in [
            "127.0.0.1",
            "192.168.1.20",
            "[::1]",
            "localhost",
            "LOCALHOST",
            "app.localhost",
        ] {
            assert!(allowed.allows(host), "{host}");
        }
    }

    #[test]
    fn other_names_need_to_be_allowed() {
        let allowed = AllowedHosts::new(["NAS.local".to_owned()]);
        assert!(allowed.allows("nas.local"));
        assert!(allowed.allows("nas.local."));
        for host in ["evil.example", "localhost.evil.example", "2130706433", ""] {
            assert!(!allowed.allows(host), "{host}");
        }
    }
}
