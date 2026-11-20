//! Assembles the HTTP application: the API, the UI, and what guards both.

use axum::http::{HeaderValue, header};
use axum::response::Response;
use axum::{Router, middleware};
use binge_core::{Accounts, Backups, Catalog, Database};
use tower_http::trace::TraceLayer;

use crate::host::AllowedHosts;
use crate::update::Updates;
use crate::{api, host, ui};

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub catalog: Catalog,
    pub accounts: Accounts,
    pub backups: Backups,
    pub updates: Updates,
    pub allowed_hosts: AllowedHosts,
}

pub fn router(state: AppState) -> Router {
    api::mount(Router::new(), &state)
        .fallback(ui::serve)
        .layer(middleware::from_fn_with_state(
            state.allowed_hosts.clone(),
            host::check,
        ))
        .with_state(state)
        .layer(middleware::map_response(not_in_a_frame))
        .layer(TraceLayer::new_for_http())
}

/// Tells browsers not to show anything of the server inside another page.
///
/// A browser sends the session cookie from a frame when the page around it
/// is on another port of the same machine, so such a page could put the
/// signed-in app, unseen, under someone's clicks.
async fn not_in_a_frame(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("frame-ancestors 'none'"),
    );
    response
}

#[cfg(test)]
mod tests;
