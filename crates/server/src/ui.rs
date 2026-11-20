//! Serves the web UI embedded in the binary.

use axum::http::{Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::{EmbeddedFile, RustEmbed};

#[derive(RustEmbed)]
#[folder = "$OUT_DIR/ui"]
struct Ui;

const INDEX: &str = "index.html";

/// Folder the bundler writes its files to, each with a content hash in its
/// name.
const ASSETS: &str = "assets/";

/// Answers every request outside the API.
///
/// Files that exist are served as they are. A path that can only mean a file
/// and does not exist is not found. Any other path gets the app's entry page,
/// because the client-side router owns those URLs.
pub async fn serve(method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, "GET, HEAD")],
        )
            .into_response();
    }

    let path = uri.path().trim_start_matches('/');
    if !path.is_empty()
        && let Some(file) = Ui::get(path)
    {
        return respond(path, file);
    }
    if names_a_file(path) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match Ui::get(INDEX) {
        Some(index) => respond(INDEX, index),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Whether a path can only mean a file, never a page of the app: anything
/// under the assets folder, and anything whose last segment has an extension.
///
/// Answering these with the entry page would hand a browser HTML where it
/// asked for a script, for example in a tab left open across an upgrade.
fn names_a_file(path: &str) -> bool {
    path.starts_with(ASSETS)
        || path
            .rsplit('/')
            .next()
            .is_some_and(|last| last.contains('.'))
}

fn respond(path: &str, file: EmbeddedFile) -> Response {
    // Hashed names never change. Everything else must be revalidated to pick
    // up new builds.
    let cache = if path.starts_with(ASSETS) {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let headers = [
        (header::CONTENT_TYPE, file.metadata.mimetype()),
        (header::CACHE_CONTROL, cache),
    ];
    // Release builds hold the file as static bytes, served without a copy.
    (headers, file.data).into_response()
}
