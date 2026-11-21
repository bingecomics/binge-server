//! Tests of the whole application through its HTTP interface.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use binge_core::fixtures;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::*;
use crate::update;

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }

    fn content_type(&self) -> &str {
        self.headers
            .get(header::CONTENT_TYPE)
            .map(|value| value.to_str().unwrap())
            .unwrap_or_default()
    }
}

/// What the owner of the test servers signs in with.
const OWNER: &str = "Ana";
const PASSWORD: &str = "correct horse";

/// A server with an empty data directory and a folder to put comics in, and
/// who the requests sent to it are made as. Clones talk to the same server.
#[derive(Clone)]
struct Server {
    app: Router,
    catalog: Catalog,
    comics: PathBuf,
    who: Who,
    _dir: Arc<tempfile::TempDir>,
}

/// What a request shows of who makes it.
#[derive(Clone)]
enum Who {
    Nobody,
    /// The `name=value` of a session cookie.
    Session(String),
    /// The secret of an API token.
    Token(String),
}

/// A server that was set up, with requests made as its owner.
async fn server() -> Server {
    let server = new_server().await;
    let owner = json!({ "name": OWNER, "password": PASSWORD });
    let set_up = server.post("/api/v1/setup", owner).await;
    assert_eq!(set_up.status, StatusCode::CREATED);
    server.with_session_of(&set_up)
}

/// A server nobody has set up yet.
async fn new_server() -> Server {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let comics = dir.path().join("Comics");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&comics).unwrap();
    let db = Database::open(&data.join("binge.db")).await.unwrap();
    let catalog = Catalog::new(db.clone(), &data);
    let state = AppState {
        accounts: Accounts::new(db.clone()).without_waits(),
        backups: binge_core::Backups::new(db.clone(), &data),
        updates: Updates::new(db.clone(), update::Setup::nowhere(&data)),
        db,
        catalog: catalog.clone(),
        allowed_hosts: AllowedHosts::new(["nas.local".to_owned()]),
    };
    Server {
        app: router(state),
        catalog,
        comics: comics.canonicalize().unwrap(),
        who: Who::Nobody,
        _dir: Arc::new(dir),
    }
}

impl Server {
    /// The same server, asked by someone who shows `who`.
    fn asked_as(&self, who: Who) -> Self {
        Self {
            who,
            ..self.clone()
        }
    }

    /// The same server, asked by someone who shows nothing.
    fn signed_out(&self) -> Self {
        self.asked_as(Who::Nobody)
    }

    /// The same server, asked with the session an answer has just set.
    fn with_session_of(&self, answer: &Answer) -> Self {
        let set = answer.headers[header::SET_COOKIE].to_str().unwrap();
        let cookie = set.split(';').next().unwrap().to_owned();
        assert!(cookie.starts_with("binge_session="), "{set}");
        self.asked_as(Who::Session(cookie))
    }

    /// The same server, asked with an API token.
    fn with_token(&self, secret: &str) -> Self {
        self.asked_as(Who::Token(secret.to_owned()))
    }

    /// The same server, asked by someone who has just signed in.
    async fn signed_in_as(&self, name: &str, password: &str) -> Self {
        let credentials = json!({ "name": name, "password": password });
        let answer = self.signed_out().post("/api/v1/sign-in", credentials).await;
        assert_eq!(answer.status, StatusCode::OK, "{name}");
        self.with_session_of(&answer)
    }

    async fn send(&self, mut request: Request<Body>) -> Answer {
        let shown = match &self.who {
            Who::Nobody => None,
            Who::Session(cookie) => Some((header::COOKIE, cookie.clone())),
            Who::Token(secret) => Some((header::AUTHORIZATION, format!("Bearer {secret}"))),
        };
        if let Some((name, value)) = shown {
            request.headers_mut().insert(name, value.parse().unwrap());
        }
        let response = self.app.clone().oneshot(request).await.unwrap();
        Answer {
            status: response.status(),
            headers: response.headers().clone(),
            body: response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        }
    }

    async fn call(&self, method: Method, path: &str) -> Answer {
        let request = Request::builder().method(method).uri(path);
        self.send(request.body(Body::empty()).unwrap()).await
    }

    async fn get(&self, path: &str) -> Answer {
        self.call(Method::GET, path).await
    }

    async fn get_as_host(&self, host: &str) -> Answer {
        let request = Request::get("/api/v1/health").header(header::HOST, host);
        self.send(request.body(Body::empty()).unwrap()).await
    }

    async fn send_json(&self, method: Method, path: &str, body: Value) -> Answer {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json");
        self.send(request.body(Body::from(body.to_string())).unwrap())
            .await
    }

    async fn post(&self, path: &str, body: Value) -> Answer {
        self.send_json(Method::POST, path, body).await
    }

    /// Adds the comics folder as a library, waits for its scan, and returns
    /// its id.
    async fn add_library(&self) -> i64 {
        self.add_library_at(&self.comics).await
    }

    /// Adds a folder as a library, waits for its scan, and returns its id.
    async fn add_library_at(&self, folder: &Path) -> i64 {
        let added = self
            .post("/api/v1/libraries", json!({ "path": folder }))
            .await;
        assert_eq!(added.status, StatusCode::CREATED);
        let id = added.json()["id"].as_i64().unwrap();
        self.catalog.scan_finished(id).await;
        id
    }
}

#[tokio::test]
async fn health_reports_version_and_database() {
    let answer = server().await.get("/api/v1/health").await;
    assert_eq!(answer.status, StatusCode::OK);
    let health = answer.json();
    assert_eq!(health["name"], "Binge Server");
    assert_eq!(health["version"], binge_core::VERSION);
    assert_eq!(health["database"], "ok");
    assert_eq!(health["needs_setup"], false);
}

#[tokio::test]
async fn openapi_describes_the_routes() {
    let answer = server().await.get("/api/v1/openapi.json").await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.content_type().starts_with("application/json"));
    let description = answer.json();
    assert!(description["paths"]["/api/v1/health"]["get"].is_object());
    assert!(description["paths"]["/api/v1/libraries"]["post"].is_object());
    assert!(description["components"]["schemas"]["ApiError"].is_object());
}

#[tokio::test]
async fn unknown_api_paths_are_json_errors() {
    let server = server().await;
    for path in ["/api", "/api/", "/api/v1", "/api/v1/nope", "/api/v2/health"] {
        let answer = server.get(path).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{path}");
        assert!(
            answer.content_type().starts_with("application/json"),
            "{path}"
        );
        assert_eq!(answer.json()["code"], "not_found", "{path}");
    }
}

#[tokio::test]
async fn client_side_routes_get_the_entry_page() {
    let server = server().await;
    for path in ["/", "/settings", "/libraries/42", "/apiary"] {
        let answer = server.get(path).await;
        assert_eq!(answer.status, StatusCode::OK, "{path}");
        assert!(answer.content_type().starts_with("text/html"), "{path}");
        assert!(
            String::from_utf8(answer.body)
                .unwrap()
                .contains("<!doctype html>")
        );
    }
}

#[tokio::test]
async fn missing_files_are_not_found() {
    let server = server().await;
    for path in [
        "/assets/gone-1a2b3c.js",
        "/favicon.ico",
        "/library/cover.png",
    ] {
        assert_eq!(
            server.get(path).await.status,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
}

#[tokio::test]
async fn wrong_method_on_an_api_route_is_a_json_error() {
    let answer = server().await.call(Method::POST, "/api/v1/health").await;
    assert_eq!(answer.status, StatusCode::METHOD_NOT_ALLOWED);
    assert!(answer.content_type().starts_with("application/json"));
    assert!(answer.headers.contains_key(header::ALLOW));
    let error = answer.json();
    assert_eq!(error["code"], "method_not_allowed");
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    );
}

#[tokio::test]
async fn the_ui_only_answers_reads() {
    let answer = server().await.call(Method::POST, "/settings").await;
    assert_eq!(answer.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(answer.headers[header::ALLOW], "GET, HEAD");
}

#[tokio::test]
async fn known_hosts_are_answered() {
    let server = server().await;
    for host in [
        "localhost:8686",
        "127.0.0.1:8686",
        "[::1]:8686",
        "192.168.1.20",
        "nas.local:8686",
    ] {
        assert_eq!(
            server.get_as_host(host).await.status,
            StatusCode::OK,
            "{host}"
        );
    }
}

#[tokio::test]
async fn unknown_hosts_are_refused() {
    let server = server().await;
    for host in ["evil.example", "evil.example:8686", "not a host"] {
        let answer = server.get_as_host(host).await;
        assert_eq!(answer.status, StatusCode::FORBIDDEN, "{host}");
        assert_eq!(answer.json()["code"], "host_not_allowed", "{host}");
    }
}

#[tokio::test]
async fn a_library_can_be_added_browsed_and_removed() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("Saga/Saga 2.cbz"), 3, 2);
    fixtures::write_book(&server.comics.join("Saga/Saga 10.cbz"), 2, 10);
    fixtures::write_book(&server.comics.join("One Shot.cbz"), 1, 30);
    assert_eq!(server.get("/api/v1/libraries").await.json(), json!([]));

    let id = server.add_library().await;

    let library = server.get(&format!("/api/v1/libraries/{id}")).await.json();
    assert_eq!(library["name"], "Comics");
    assert_eq!(library["path"], server.comics.to_str().unwrap());
    assert_eq!(library["series_count"], 2);
    assert_eq!(library["book_count"], 3);
    assert_eq!(library["scan"]["phase"], "idle");
    assert_eq!(library["scan"]["analyzed"], 3);
    assert!(library["last_scanned_at"].is_i64());
    assert_eq!(server.get("/api/v1/libraries").await.json()[0]["id"], id);

    let series = server
        .get(&format!("/api/v1/libraries/{id}/series"))
        .await
        .json()["series"]
        .clone();
    // The file directly in the folder is a series of its own.
    assert_eq!(series.as_array().unwrap().len(), 2);
    assert_eq!(series[0]["name"], "One Shot");
    assert_eq!(series[0]["book_count"], 1);
    assert!(series[0]["cover"].is_string());
    let series = &series[1];
    assert_eq!(series["name"], "Saga");
    assert_eq!(series["book_count"], 2);
    let series_id = series["id"].as_i64().unwrap();
    assert_eq!(
        server
            .get(&format!("/api/v1/series/{series_id}"))
            .await
            .json(),
        *series
    );

    let books = server
        .get(&format!("/api/v1/series/{series_id}/books"))
        .await
        .json();
    assert_eq!(books[0]["name"], "Saga 2");
    assert_eq!(books[1]["name"], "Saga 10");
    assert_eq!(books[0]["page_count"], 3);
    assert_eq!(books[0]["cover"], books[0]["identity"]);
    assert_eq!(series["cover"], books[0]["cover"]);
    assert!(books[0]["problem"].is_null());
    assert_eq!(books[0]["series_id"], series_id);
    // Where a book sits is the server's own record: here, what its name said.
    assert_eq!(books[0]["number"], "2");
    assert!(books[0]["group_id"].is_null());

    let cover = books[0]["cover"].as_str().unwrap();
    let image = server.get(&format!("/api/v1/covers/{cover}")).await;
    assert_eq!(image.status, StatusCode::OK);
    assert_eq!(image.content_type(), "image/jpeg");
    assert!(
        image.headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    assert!(image.body.starts_with(&[0xff, 0xd8]));

    let removed = server
        .call(Method::DELETE, &format!("/api/v1/libraries/{id}"))
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    assert_eq!(server.get("/api/v1/libraries").await.json(), json!([]));
    assert_eq!(
        server.get(&format!("/api/v1/covers/{cover}")).await.status,
        StatusCode::NOT_FOUND
    );
    assert!(server.comics.join("Saga/Saga 2.cbz").is_file());
}

#[tokio::test]
async fn a_rescan_is_started_on_request() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("Saga/Saga 1.cbz"), 1, 1);
    let id = server.add_library().await;

    fixtures::write_book(&server.comics.join("Saga/Saga 2.cbz"), 1, 2);
    let started = server
        .call(Method::POST, &format!("/api/v1/libraries/{id}/scan"))
        .await;
    assert_eq!(started.status, StatusCode::ACCEPTED);
    assert_ne!(started.json()["scan"]["phase"], "idle");

    server.catalog.scan_finished(id).await;
    let library = server.get(&format!("/api/v1/libraries/{id}")).await.json();
    assert_eq!(library["book_count"], 2);
}

#[tokio::test]
async fn a_library_that_cannot_be_added_says_why() {
    let server = server().await;
    let missing = server.comics.join("missing");

    let bad_path = server
        .post("/api/v1/libraries", json!({ "path": missing }))
        .await;
    assert_eq!(bad_path.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad_path.json()["code"], "invalid_request");
    assert_eq!(
        bad_path.json()["message"],
        "There is no folder at that path."
    );

    let relative = server
        .post("/api/v1/libraries", json!({ "path": "Comics" }))
        .await;
    assert_eq!(relative.status, StatusCode::BAD_REQUEST);

    server.add_library().await;
    let twice = server
        .post("/api/v1/libraries", json!({ "path": server.comics }))
        .await;
    assert_eq!(twice.status, StatusCode::CONFLICT);
    assert_eq!(twice.json()["code"], "conflict");

    // A body axum rejects by itself still comes back in the API's shape.
    let no_path = server
        .post("/api/v1/libraries", json!({ "name": "Comics" }))
        .await;
    assert!(no_path.status.is_client_error());
    assert_eq!(no_path.json()["code"], "invalid_request");
}

#[tokio::test]
async fn unknown_ids_and_covers_are_not_found() {
    let server = server().await;
    let paths = [
        "/api/v1/libraries/9".to_owned(),
        "/api/v1/libraries/9/series".to_owned(),
        "/api/v1/libraries/9/unreadable".to_owned(),
        "/api/v1/series/9".to_owned(),
        "/api/v1/series/9/books".to_owned(),
        "/api/v1/series/9/overview".to_owned(),
        "/api/v1/groups/9/overview".to_owned(),
        "/api/v1/groups/9/books".to_owned(),
        format!("/api/v1/covers/{}", "0".repeat(64)),
        "/api/v1/covers/..%2F..%2Fbinge.db".to_owned(),
    ];
    for path in paths {
        let answer = server.get(&path).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(answer.json()["code"], "not_found", "{path}");
    }
    let scan = server.call(Method::POST, "/api/v1/libraries/9/scan").await;
    assert_eq!(scan.status, StatusCode::NOT_FOUND);
    let remove = server.call(Method::DELETE, "/api/v1/libraries/9").await;
    assert_eq!(remove.status, StatusCode::NOT_FOUND);
}

impl Server {
    async fn put(&self, path: &str, body: Value) -> Answer {
        self.send_json(Method::PUT, path, body).await
    }

    /// The first book of a library's first series: its only book, in the
    /// tests that put one file in the library folder.
    async fn first_book(&self, library_id: i64) -> Value {
        let series = self
            .get(&format!("/api/v1/libraries/{library_id}/series"))
            .await
            .json()["series"]
            .clone();
        let series_id = series[0]["id"].as_i64().unwrap();
        let books = self
            .get(&format!("/api/v1/series/{series_id}/books"))
            .await
            .json();
        books[0].clone()
    }
}

#[tokio::test]
async fn a_book_is_read_page_by_page() {
    let server = server().await;
    let (first, second) = (fixtures::page(40, 60, 1), fixtures::page(60, 40, 2));
    fixtures::write_cbz(
        &server.comics.join("One Shot.cbz"),
        &[("01.png", first.clone()), ("02.png", second.clone())],
    );
    let library_id = server.add_library().await;
    let listed = server.first_book(library_id).await;
    let (id, identity) = (
        listed["id"].as_i64().unwrap(),
        listed["identity"].as_str().unwrap(),
    );

    let book = server.get(&format!("/api/v1/books/{id}")).await.json();
    assert_eq!(book, listed);
    assert_eq!(book["library_id"], library_id);
    assert!(book["progress"].is_null());

    let pages = server
        .get(&format!("/api/v1/books/{id}/pages"))
        .await
        .json();
    assert_eq!(pages.as_array().unwrap().len(), 2);
    assert_eq!(pages[0]["number"], 0);
    assert_eq!(pages[0]["byte_size"], first.len());
    assert_eq!(
        (&pages[0]["width"], &pages[0]["height"]),
        (&json!(40), &json!(60))
    );
    assert_eq!(
        (&pages[1]["width"], &pages[1]["height"]),
        (&json!(60), &json!(40))
    );

    let address = format!("/api/v1/pages/{identity}/1");
    let image = server.get(&address).await;
    assert_eq!(image.status, StatusCode::OK);
    assert_eq!(image.content_type(), "image/png");
    assert_eq!(image.body, second);
    assert_eq!(
        image.headers[header::CONTENT_LENGTH],
        second.len().to_string()
    );
    assert_eq!(
        image.headers[header::CACHE_CONTROL],
        "private, max-age=31536000, immutable"
    );

    // A client that holds the image is told so without it being sent again.
    let etag = image.headers[header::ETAG].clone();
    let again = Request::get(&address).header(header::IF_NONE_MATCH, &etag);
    let again = server.send(again.body(Body::empty()).unwrap()).await;
    assert_eq!(again.status, StatusCode::NOT_MODIFIED);
    assert!(again.body.is_empty());
    assert_eq!(again.headers[header::ETAG], etag);
    let other = Request::get(&address).header(header::IF_NONE_MATCH, "\"something else\"");
    let other = server.send(other.body(Body::empty()).unwrap()).await;
    assert_eq!(other.status, StatusCode::OK);
}

#[tokio::test]
async fn a_page_that_cannot_be_served_says_why() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("One Shot.cbz"), 2, 1);
    let library_id = server.add_library().await;
    let book = server.first_book(library_id).await;
    let identity = book["identity"].as_str().unwrap();

    for path in [
        format!("/api/v1/pages/{identity}/2"),
        format!("/api/v1/pages/{identity}/-1"),
        format!("/api/v1/pages/{}/0", "0".repeat(64)),
        "/api/v1/pages/..%2F..%2Fbinge.db/0".to_owned(),
        "/api/v1/pages/caf%C3%A9/0".to_owned(),
        "/api/v1/books/99".to_owned(),
        "/api/v1/books/99/pages".to_owned(),
    ] {
        let answer = server.get(&path).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(answer.json()["code"], "not_found", "{path}");
    }
    // Naming a tag does not turn a page that is not there into one that is.
    let nobody = "0".repeat(64);
    let made_up = Request::get(format!("/api/v1/pages/{nobody}/0"))
        .header(header::IF_NONE_MATCH, format!("\"{nobody}-0\""));
    let made_up = server.send(made_up.body(Body::empty()).unwrap()).await;
    assert_eq!(made_up.status, StatusCode::NOT_FOUND);

    let not_a_number = server.get(&format!("/api/v1/pages/{identity}/first")).await;
    assert_eq!(not_a_number.status, StatusCode::BAD_REQUEST);
    assert_eq!(not_a_number.json()["code"], "invalid_request");

    // Other pages under the same file name, not scanned yet.
    let address = format!("/api/v1/pages/{identity}/0");
    fixtures::write_book(&server.comics.join("One Shot.cbz"), 2, 9);
    let changed = server.get(&address).await;
    assert_eq!(changed.status, StatusCode::CONFLICT);
    assert_eq!(changed.json()["code"], "conflict");

    std::fs::remove_file(server.comics.join("One Shot.cbz")).unwrap();
    let gone = server.get(&address).await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
    assert!(
        gone.json()["message"]
            .as_str()
            .unwrap()
            .contains("not where it was")
    );
}

#[tokio::test]
async fn progress_is_saved_and_comes_back_with_the_book() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("One Shot.cbz"), 3, 1);
    let library_id = server.add_library().await;
    let id = server.first_book(library_id).await["id"].as_i64().unwrap();
    let address = format!("/api/v1/books/{id}/progress");

    let saved = server.put(&address, json!({ "page": 1 })).await;
    assert_eq!(saved.status, StatusCode::OK);
    assert_eq!(saved.json()["page"], 1);
    assert_eq!(saved.json()["completed"], false);
    assert!(saved.json()["updated_at"].is_i64());
    assert_eq!(
        server.first_book(library_id).await["progress"],
        saved.json()
    );
    let book = server.get(&format!("/api/v1/books/{id}")).await.json();
    assert_eq!(book["progress"], saved.json());

    let finished = server.put(&address, json!({ "page": 2 })).await;
    assert_eq!(finished.json()["completed"], true);

    for outside in [json!({ "page": 3 }), json!({ "page": -1 })] {
        let refused = server.put(&address, outside).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST);
        assert_eq!(refused.json()["code"], "invalid_request");
        assert_eq!(refused.json()["message"], "That page is not in this book.");
    }
    let no_page = server.put(&address, json!({})).await;
    assert!(no_page.status.is_client_error());
    assert_eq!(no_page.json()["code"], "invalid_request");
    let no_book = server
        .put("/api/v1/books/99/progress", json!({ "page": 0 }))
        .await;
    assert_eq!(no_book.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_series_is_seen_whole_and_says_what_to_read_next() {
    let server = server().await;
    let pages = || {
        vec![
            ("01.png", fixtures::page(40, 60, 1)),
            ("02.png", fixtures::page(40, 60, 2)),
        ]
    };
    let mut first = pages();
    first.push((
        "ComicInfo.xml",
        fixtures::comic_info(&[
            ("Writer", "Ana Pérez, Ben Okri"),
            ("Publisher", "Small Press"),
            ("Year", "1988"),
            ("Summary", "It gets dark."),
        ]),
    ));
    fixtures::write_cbz(&server.comics.join("Saga/Saga 1.cbz"), &first);
    let mut second = pages();
    second[0].1 = fixtures::page(40, 60, 3);
    second.push((
        "ComicInfo.xml",
        fixtures::comic_info(&[("Writer", "Ana Pérez"), ("Year", "1990")]),
    ));
    fixtures::write_cbz(&server.comics.join("Saga/Saga 2.cbz"), &second);
    let library_id = server.add_library().await;
    let series = server
        .get(&format!("/api/v1/libraries/{library_id}/series"))
        .await
        .json()["series"][0]
        .clone();
    let address = format!("/api/v1/series/{}/overview", series["id"]);
    let books = server
        .get(&format!("/api/v1/series/{}/books", series["id"]))
        .await
        .json();

    let unread = server.get(&address).await;
    assert_eq!(unread.status, StatusCode::OK);
    assert_eq!(
        unread.json(),
        json!({
            "series": series,
            "groups": [],
            "read": {
                "readable_count": 2,
                "page_count": 4,
                "books_read": 0,
                "pages_read": 0,
                // Nobody read enough here for a pace of their own.
                "seconds_left": null,
                "next": books[0],
            },
            "publisher": "Small Press",
            "first_year": 1988,
            "last_year": 1990,
            "writers": [{ "id": 1, "name": "Ana Pérez" }, { "id": 2, "name": "Ben Okri" }],
            "pencillers": [],
            "summary": { "text": "It gets dark.", "book": books[0] },
        })
    );

    // Finishing the first book moves on to the second.
    let finished = server
        .put(
            &format!("/api/v1/books/{}/progress", books[0]["id"]),
            json!({ "page": 1 }),
        )
        .await;
    assert_eq!(finished.json()["completed"], true);
    let begun = server.get(&address).await.json();
    assert_eq!(begun["read"]["books_read"], 1);
    assert_eq!(begun["read"]["pages_read"], 2);
    assert_eq!(begun["read"]["next"]["id"], books[1]["id"]);
}

#[tokio::test]
async fn a_file_that_cannot_be_read_is_listed_apart_from_the_series() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("Saga/Saga 1.cbz"), 1, 1);
    std::fs::write(server.comics.join("Saga/Saga 2.cbz"), "not a zip").unwrap();
    let id = server.add_library().await;

    let series = server
        .get(&format!("/api/v1/libraries/{id}/series"))
        .await
        .json()["series"]
        .clone();
    assert_eq!(series[0]["book_count"], 1);
    let books = server
        .get(&format!("/api/v1/series/{}/books", series[0]["id"]))
        .await
        .json();
    assert_eq!(books.as_array().unwrap().len(), 1);

    let files = server
        .get(&format!("/api/v1/libraries/{id}/unreadable"))
        .await
        .json();
    assert_eq!(files.as_array().unwrap().len(), 1);
    assert_eq!(files[0]["name"], "Saga 2");
    assert_eq!(files[0]["series_id"], json!(null));
    assert_eq!(
        files[0]["problem"],
        "This file is not a readable CBZ archive."
    );
}

#[tokio::test]
async fn the_owner_lays_out_a_library_and_is_told_what_is_in_the_way() {
    let server = server().await;
    for number in 1..=2 {
        let file = format!("Saga/Saga {number}.cbz");
        fixtures::write_book(&server.comics.join(file), 1, number);
    }
    let library = server.add_library().await;
    let saga = server.first_book(library).await["series_id"].clone();
    let in_library = format!("/api/v1/libraries/{library}/series");

    // A new series is there at once, with no books in it.
    let made = server
        .post(&in_library, json!({ "name": " Odd Squad " }))
        .await;
    assert_eq!(made.status, StatusCode::CREATED);
    let squad = made.json();
    assert_eq!(squad["name"], "Odd Squad");
    assert_eq!(squad["book_count"], 0);
    let squad = squad["id"].as_i64().unwrap();
    let listed = server.get(&in_library).await.json();
    assert_eq!(listed["total"], 2);
    let empty = server.get(&format!("/api/v1/series/{squad}/books")).await;
    assert_eq!(empty.json(), json!([]));
    let overview = server
        .get(&format!("/api/v1/series/{squad}/overview"))
        .await;
    assert_eq!(overview.status, StatusCode::OK);
    assert_eq!(overview.json()["read"]["next"], json!(null));

    // A name that is taken is refused, and the answer says by which series.
    // Nothing is merged.
    let taken = server.post(&in_library, json!({ "name": "SAGA" })).await;
    assert_refused(&taken, StatusCode::CONFLICT, "conflict", "create");
    assert_eq!(taken.json()["conflicts_with"], saga);
    let a_squad = format!("/api/v1/series/{squad}");
    let taken = server.patch(&a_squad, json!({ "name": "saga" })).await;
    assert_refused(&taken, StatusCode::CONFLICT, "conflict", "rename");
    assert_eq!(taken.json()["conflicts_with"], saga);
    let unnamed = server.patch(&a_squad, json!({ "name": "  " })).await;
    assert_refused(&unnamed, StatusCode::BAD_REQUEST, "invalid_request", "name");
    assert!(unnamed.json().get("conflicts_with").is_none());
    let renamed = server.patch(&a_squad, json!({ "name": "The Squad" })).await;
    assert_eq!(renamed.status, StatusCode::OK);
    assert_eq!(renamed.json()["name"], "The Squad");

    // Groups: made, told apart, renamed.
    let groups = format!("/api/v1/series/{squad}/groups");
    let made = server
        .post(&groups, json!({ "kind": "volume", "number": 1 }))
        .await;
    assert_eq!(made.status, StatusCode::CREATED);
    let volume = made.json();
    assert_eq!(volume["series_id"], squad);
    assert_eq!(volume["book_count"], 0);
    let volume = volume["id"].as_i64().unwrap();
    let again = server
        .post(&groups, json!({ "kind": "volume", "number": 1 }))
        .await;
    assert_refused(&again, StatusCode::CONFLICT, "conflict", "same group");
    assert_eq!(again.json()["conflicts_with"], volume);
    let unfit = server
        .post(&groups, json!({ "kind": "volume", "number": 0 }))
        .await;
    assert_refused(&unfit, StatusCode::BAD_REQUEST, "invalid_request", "volume");
    let unknown = server
        .post(&groups, json!({ "kind": "arc", "name": "x" }))
        .await;
    assert!(unknown.status.is_client_error());
    let a_volume = format!("/api/v1/groups/{volume}");
    let second = server.patch(&a_volume, json!({ "number": 2 })).await;
    assert_eq!(second.json()["number"], 2);
    let listed = server.get(&format!("{a_squad}/overview")).await.json();
    assert_eq!(listed["groups"][0]["id"], volume);

    // Every kind of group takes the same parts: a number, a name, both or
    // neither. A series holds as many of a kind as are told apart by them.
    let mut made = Vec::new();
    for group in [
        json!({ "kind": "annual", "number": 2 }),
        json!({ "kind": "annual" }),
        json!({ "kind": "one_shot" }),
        json!({ "kind": "appearance" }),
        json!({ "kind": "appearance", "name": "Strange Tales" }),
        json!({ "kind": "appearance", "name": "Strange Tales", "number": 2 }),
    ] {
        let answer = server.post(&groups, group.clone()).await;
        assert_eq!(answer.status, StatusCode::CREATED, "{group}");
        let answer = answer.json();
        for part in ["kind", "number", "name"] {
            let asked = group.get(part).unwrap_or(&Value::Null);
            assert_eq!(&answer[part], asked, "{group}");
        }
        made.push(answer["id"].as_i64().unwrap());
        let again = server.post(&groups, group).await;
        assert_refused(&again, StatusCode::CONFLICT, "conflict", "same group");
    }
    for unfit in [
        json!({ "kind": "annual", "number": 65536 }),
        json!({ "kind": "appearance", "name": " " }),
        json!({ "kind": "specials" }),
    ] {
        let answer = server.post(&groups, unfit.clone()).await;
        assert!(answer.status.is_client_error(), "{unfit}");
    }
    let listed = server.get(&format!("{a_squad}/overview")).await.json();
    let kinds: Vec<_> = listed["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| (group["kind"].as_str().unwrap(), group["number"].as_i64()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("volume", Some(2)),
            ("annual", Some(2)),
            ("annual", None),
            ("one_shot", None),
            ("appearance", Some(2)),
            ("appearance", None),
            ("appearance", None),
        ]
    );
    for id in made {
        let gone = server.delete(&format!("/api/v1/groups/{id}")).await;
        assert_eq!(gone.status, StatusCode::NO_CONTENT);
    }

    // A book goes where the place says, all of it.
    let book = server.first_book(library).await;
    let place = format!("/api/v1/books/{}/place", book["id"]);
    let whole = json!({
        "series_id": squad, "group_id": volume, "number": "7", "published": "1991-05",
    });
    let moved = server.put(&place, whole).await;
    assert_eq!(moved.status, StatusCode::OK);
    let moved = moved.json();
    assert_eq!(moved["id"], book["id"]);
    assert_eq!(moved["series_id"], squad);
    assert_eq!(moved["group_id"], volume);
    assert_eq!(moved["number"], "7");
    assert_eq!(moved["published"], "1991-05");
    let there = server.get(&format!("{a_squad}/books")).await.json();
    assert_eq!(there.as_array().unwrap().len(), 1);
    let left = server.get(&format!("/api/v1/series/{saga}/books")).await;
    assert_eq!(left.json().as_array().unwrap().len(), 1);

    for (wrong, what) in [
        (json!({ "series_id": 999 }), "no such series"),
        (
            json!({ "series_id": saga, "group_id": volume }),
            "group elsewhere",
        ),
        (
            json!({ "series_id": squad, "group_id": 999 }),
            "no such group",
        ),
        (
            json!({ "series_id": squad, "published": "May 1991" }),
            "date",
        ),
    ] {
        let refused = server.put(&place, wrong).await;
        assert_refused(&refused, StatusCode::BAD_REQUEST, "invalid_request", what);
    }
    let no_series = server.put(&place, json!({ "number": "1" })).await;
    assert!(no_series.status.is_client_error());
    // A part that is misspelt is refused, not read as left out.
    let misspelt = json!({ "series_id": squad, "groupId": volume });
    let misspelt = server.put(&place, misspelt).await;
    assert_refused(
        &misspelt,
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_request",
        "field",
    );
    let misspelt = server.patch(&a_volume, json!({ "nmuber": 3 })).await;
    assert!(misspelt.status.is_client_error());
    let still = server.get(&format!("/api/v1/books/{}", book["id"])).await;
    assert_eq!(still.json()["series_id"], squad);

    // What holds books is not deleted. What holds none is.
    for path in [&a_volume, &a_squad] {
        let kept = server.delete(path).await;
        assert_refused(&kept, StatusCode::CONFLICT, "conflict", path);
    }
    let back = server.put(&place, json!({ "series_id": saga })).await;
    assert_eq!(back.json()["group_id"], json!(null));
    assert_eq!(back.json()["number"], json!(null));
    for path in [&a_volume, &a_squad] {
        assert_eq!(server.delete(path).await.status, StatusCode::NO_CONTENT);
        let gone = server.delete(path).await;
        assert_refused(&gone, StatusCode::NOT_FOUND, "not_found", path);
    }
    let listed = server.get(&in_library).await.json();
    assert_eq!(listed["total"], 1);

    // An AI client does the same with one of the owner's tokens, and is
    // refused the same way.
    let (_, secret) = server.make_token("AI client").await;
    let client = server.with_token(&secret);
    let made = client
        .post(&in_library, json!({ "name": "By Token" }))
        .await;
    assert_eq!(made.status, StatusCode::CREATED);
    let taken = client
        .post(&in_library, json!({ "name": "by token" }))
        .await;
    assert_refused(&taken, StatusCode::CONFLICT, "conflict", "token");
    assert_eq!(taken.json()["conflicts_with"], made.json()["id"]);

    let nowhere = [
        (
            Method::POST,
            "/api/v1/libraries/9/series",
            json!({ "name": "x" }),
        ),
        (Method::PATCH, "/api/v1/series/9", json!({ "name": "x" })),
        (
            Method::POST,
            "/api/v1/series/9/groups",
            json!({ "kind": "one_shot" }),
        ),
        (Method::PATCH, "/api/v1/groups/9", json!({ "name": "x" })),
        (
            Method::PUT,
            "/api/v1/books/9/place",
            json!({ "series_id": saga }),
        ),
    ];
    for (method, path, body) in nowhere {
        let answer = server.send_json(method, path, body).await;
        assert_refused(&answer, StatusCode::NOT_FOUND, "not_found", path);
    }
}

#[tokio::test]
async fn the_first_book_in_reading_order_stands_for_its_series() {
    let server = server().await;
    // In file name order: a special, then a later volume, then the first run.
    let books = [
        ("Patrol/A special.cbz", 1, vec![("Year", "1963")]),
        (
            "Patrol/Patrol v2 001.cbz",
            2,
            vec![("Number", "1"), ("Year", "1987")],
        ),
        (
            "Patrol/Patrol v1 086.cbz",
            3,
            vec![
                ("Number", "86"),
                ("Year", "1964"),
                ("Summary", "It begins."),
            ],
        ),
    ];
    for (path, seed, said) in &books {
        fixtures::write_tagged_book(&server.comics.join(path), *seed, said);
    }
    let library_id = server.add_library().await;
    let series = server
        .get(&format!("/api/v1/libraries/{library_id}/series"))
        .await
        .json()["series"][0]
        .clone();
    let listed = server
        .get(&format!("/api/v1/series/{}/books", series["id"]))
        .await
        .json();
    let first = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|book| book["name"] == "Patrol v1 086")
        .unwrap();

    assert_eq!(series["cover"], first["cover"]);
    let overview = server
        .get(&format!("/api/v1/series/{}/overview", series["id"]))
        .await
        .json();
    assert_eq!(overview["read"]["next"]["id"], first["id"]);
    assert_eq!(overview["summary"]["book"]["id"], first["id"]);

    // The books are listed as they are read, group by group, and the groups
    // come with the series.
    let names: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|book| book["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Patrol v1 086", "Patrol v2 001", "A special"]);
    let groups: Vec<_> = overview["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| (&group["kind"], &group["number"], &group["book_count"]))
        .collect();
    assert_eq!(
        groups,
        [
            (&json!("volume"), &json!(1), &json!(1)),
            (&json!("volume"), &json!(2), &json!(1)),
            (&json!("one_shot"), &json!(null), &json!(1)),
        ]
    );
    assert_eq!(listed[0]["group_id"], overview["groups"][0]["id"]);
    assert_eq!(listed[2]["group_id"], overview["groups"][2]["id"]);
}

#[tokio::test]
async fn a_book_is_followed_by_the_next_where_it_is_read() {
    let server = server().await;
    let comics = [
        "Patrol/Patrol v1 001.cbz",
        "Patrol/Patrol v1 002.cbz",
        "Patrol/Patrol v2 001.cbz",
        "Saga/Saga 001.cbz",
    ];
    for (seed, path) in comics.iter().enumerate() {
        fixtures::write_book(&server.comics.join(path), 1, seed as u8 + 1);
    }
    let library_id = server.add_library().await;
    let named = async |name: &str| server.book_named(library_id, name).await;
    let (first, second, third, saga) = (
        named("Patrol v1 001").await,
        named("Patrol v1 002").await,
        named("Patrol v2 001").await,
        named("Saga 001").await,
    );
    let after = async |book: &Value, from: &str| {
        let answer = server
            .get(&format!("/api/v1/books/{}/next{from}", book["id"]))
            .await
            .json();
        (answer["followed"].clone(), answer["next"]["id"].clone())
    };

    // The series goes on from one volume to the next, and the volume ends.
    assert_eq!(
        after(&first, "").await,
        (json!("series"), second["id"].clone())
    );
    assert_eq!(
        after(&second, "").await,
        (json!("series"), third["id"].clone())
    );
    assert_eq!(after(&third, "").await, (json!("series"), json!(null)));
    assert_eq!(
        after(&first, "?group=true").await,
        (json!("group"), second["id"].clone())
    );
    assert_eq!(
        after(&second, "?group=true").await,
        (json!("group"), json!(null))
    );

    // A reading list leads from one series into another.
    let new = json!({ "name": "Crossover", "books": [second["id"], saga["id"]] });
    let list = server.post("/api/v1/reading-lists", new).await.json()["id"].clone();
    assert_eq!(
        after(&second, &format!("?list={list}&group=true")).await,
        (json!("list"), saga["id"].clone())
    );
    assert_eq!(
        after(&saga, &format!("?list={list}")).await,
        (json!("list"), json!(null))
    );

    // A book that is not in the list is a part of nothing there.
    assert_eq!(
        after(&first, &format!("?list={list}")).await,
        (json!(null), json!(null))
    );

    for gone in [
        "/api/v1/books/999/next".to_owned(),
        format!("/api/v1/books/{}/next?list=999", first["id"]),
    ] {
        assert_eq!(server.get(&gone).await.status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn a_group_is_seen_whole_and_follows_what_was_read_in_it() {
    let server = server().await;
    // What the second volume says is not the first volume's to say.
    let books = [
        (
            "Patrol/Patrol v1 001.cbz",
            vec![("Number", "1"), ("Year", "1987")],
        ),
        (
            "Patrol/Patrol v1 002.cbz",
            vec![("Number", "2"), ("Summary", "It begins.")],
        ),
        (
            "Patrol/Patrol v1 003.cbz",
            vec![("Number", "3"), ("Year", "1988"), ("Writer", "Ana Pérez")],
        ),
        (
            "Patrol/Patrol v2 001.cbz",
            vec![("Number", "1"), ("Year", "1990"), ("Writer", "Ben Okri")],
        ),
    ];
    for (seed, (path, said)) in books.iter().enumerate() {
        fixtures::write_tagged_book(&server.comics.join(path), seed as u8 + 1, said);
    }
    let library_id = server.add_library().await;
    let series = server
        .get(&format!("/api/v1/libraries/{library_id}/series"))
        .await
        .json()["series"][0]
        .clone();
    let series_address = format!("/api/v1/series/{}/overview", series["id"]);
    let groups = server.get(&series_address).await.json()["groups"].clone();
    let first_volume = format!("/api/v1/groups/{}", groups[0]["id"]);
    let second_volume = format!("/api/v1/groups/{}", groups[1]["id"]);

    // A group lists its own books, and the first with a cover stands for it.
    let listed = server.get(&format!("{first_volume}/books")).await.json();
    let names: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|book| book["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Patrol v1 001", "Patrol v1 002", "Patrol v1 003"]);
    assert_eq!(groups[0]["cover"], listed[0]["cover"]);
    assert!(groups[0]["cover"].is_string());

    let unread = server.get(&format!("{first_volume}/overview")).await;
    assert_eq!(unread.status, StatusCode::OK);
    assert_eq!(
        unread.json(),
        json!({
            "group": groups[0],
            "series": series,
            "read": {
                "readable_count": 3,
                "page_count": 3,
                "books_read": 0,
                "pages_read": 0,
                // Nobody read enough here for a pace of their own.
                "seconds_left": null,
                "next": listed[0],
            },
            "publisher": null,
            "first_year": 1987,
            "last_year": 1988,
            "writers": [{ "id": 1, "name": "Ana Pérez" }],
            "pencillers": [],
            "summary": { "text": "It begins.", "book": listed[1] },
        })
    );

    // The reader skips the first issue, reads the second, and then reads
    // the other volume. The series goes back to what was skipped; the first
    // volume goes on from where it was left.
    let later = server.get(&format!("{second_volume}/books")).await.json();
    for book in [&listed[1], &later[0]] {
        let read = server
            .put(
                &format!("/api/v1/books/{}/progress", book["id"]),
                json!({ "page": 0 }),
            )
            .await;
        assert_eq!(read.json()["completed"], true);
    }
    let whole = server.get(&series_address).await.json();
    assert_eq!(whole["read"]["next"]["id"], listed[0]["id"]);
    let first = server.get(&format!("{first_volume}/overview")).await.json();
    assert_eq!(first["read"]["books_read"], 1);
    assert_eq!(first["read"]["next"]["id"], listed[2]["id"]);
    // A group read to the end starts again.
    let second = server
        .get(&format!("{second_volume}/overview"))
        .await
        .json();
    assert_eq!(second["read"]["books_read"], 1);
    assert_eq!(second["read"]["readable_count"], 1);
    assert_eq!(second["read"]["next"]["id"], later[0]["id"]);

    // Someone who may not read the library finds no group there.
    server.add_user("Ben", &[]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    for path in [
        format!("{first_volume}/overview"),
        format!("{first_volume}/books"),
    ] {
        let hidden = ben.get(&path).await;
        assert_refused(&hidden, StatusCode::NOT_FOUND, "not_found", &path);
    }
}

#[tokio::test]
async fn books_are_marked_read_by_group_series_and_book() {
    let server = server().await;
    for (seed, name) in ["v1 001", "v1 002", "v2 001"].iter().enumerate() {
        let file = format!("Patrol/Patrol {name}.cbz");
        fixtures::write_book(&server.comics.join(file), 2, seed as u8 + 1);
    }
    let library_id = server.add_library().await;
    let series_list = format!("/api/v1/libraries/{library_id}/series");
    let series = async || server.get(&series_list).await.json()["series"][0].clone();
    let series_id = series().await["id"].clone();
    let overview = format!("/api/v1/series/{series_id}/overview");
    let groups = async || server.get(&overview).await.json()["groups"].clone();
    assert_eq!(series().await["unread_count"], 3);
    let first_volume = groups().await[0]["id"].clone();

    // A volume: its books are read, and reading goes on in the next one.
    let marked = server
        .post(
            "/api/v1/read-marks",
            json!({ "read": true, "groups": [first_volume] }),
        )
        .await;
    assert_eq!(marked.status, StatusCode::OK);
    assert_eq!(marked.json(), json!({ "books": 2 }));
    assert_eq!(series().await["unread_count"], 1);
    let volumes = groups().await;
    assert_eq!(volumes[0]["unread_count"], 0);
    assert_eq!(volumes[1]["unread_count"], 1);
    let next = server.get("/api/v1/up-next").await.json();
    assert_eq!(next[0]["name"], "Patrol v2 001");

    // Books by themselves, with one that is not there.
    let books = server
        .get(&format!("/api/v1/series/{series_id}/books"))
        .await
        .json();
    let unread = json!({ "read": false, "books": [books[0]["id"], 9999] });
    let marked = server.post("/api/v1/read-marks", unread).await;
    assert_eq!(marked.json(), json!({ "books": 1 }));
    let book = server
        .get(&format!("/api/v1/books/{}", books[0]["id"]))
        .await;
    assert!(book.json()["progress"].is_null());

    // The whole series, which nobody else has read by that.
    let whole = json!({ "read": true, "series": [series_id] });
    let marked = server.post("/api/v1/read-marks", whole.clone()).await;
    assert_eq!(marked.json(), json!({ "books": 2 }));
    assert_eq!(series().await["unread_count"], 0);
    server.add_user("Ben", &[]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    let marked = ben.post("/api/v1/read-marks", whole).await;
    assert_eq!(marked.json(), json!({ "books": 0 }));

    let how = server
        .post("/api/v1/read-marks", json!({ "books": [1] }))
        .await;
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    assert_refused(&how, unprocessable, "invalid_request", "no mark");
    let many: Vec<i64> = (0..10_001).collect();
    let too_many = server
        .post("/api/v1/read-marks", json!({ "read": true, "books": many }))
        .await;
    assert_refused(
        &too_many,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "many",
    );
}

#[tokio::test]
async fn the_owner_chooses_covers_for_a_book_and_a_series() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("Saga/Saga 1.cbz"), 3, 1);
    fixtures::write_book(&server.comics.join("Saga/Saga 2.cbz"), 3, 2);
    let library_id = server.add_library().await;
    let series_list = format!("/api/v1/libraries/{library_id}/series");
    let series = async || server.get(&series_list).await.json()["series"][0].clone();
    let series_id = series().await["id"].clone();
    let books = server
        .get(&format!("/api/v1/series/{series_id}/books"))
        .await
        .json();
    let (first, second) = (&books[0], &books[1]);
    let book = async || {
        let path = format!("/api/v1/books/{}", first["id"]);
        server.get(&path).await.json()
    };
    let cover_of = |shown: &Value| format!("/api/v1/covers/{}", shown["cover"].as_str().unwrap());
    assert_eq!(series().await["cover"], first["identity"]);
    assert_eq!(series().await["cover_chosen"], false);

    // A page of the second book, for the series.
    let series_cover = format!("/api/v1/series/{series_id}/cover");
    let page = json!({ "book_id": second["id"], "page": 2 });
    let chosen = server.put(&series_cover, page.clone()).await;
    assert_eq!(chosen.status, StatusCode::NO_CONTENT);
    let shown = series().await;
    assert_eq!(shown["cover_chosen"], true);
    assert_ne!(shown["cover"], first["identity"]);
    let chosen_cover = cover_of(&shown);
    let image = server.get(&chosen_cover).await;
    assert_eq!(image.status, StatusCode::OK);
    assert_eq!(image.headers[header::CONTENT_TYPE], "image/jpeg");

    // A picture, for the first book.
    let book_picture = format!("/api/v1/books/{}/cover/picture", first["id"]);
    let give = async |picture: Vec<u8>| {
        let request = Request::put(&book_picture).header(header::CONTENT_TYPE, "image/png");
        server
            .send(request.body(Body::from(picture)).unwrap())
            .await
    };
    let given = give(fixtures::page(300, 450, 9)).await;
    assert_eq!(given.status, StatusCode::NO_CONTENT);
    let with_picture = book().await;
    assert_eq!(with_picture["cover_chosen"], true);
    assert_ne!(with_picture["cover"], first["identity"]);
    assert_eq!(
        server.get(&cover_of(&with_picture)).await.status,
        StatusCode::OK
    );

    let bad = StatusCode::BAD_REQUEST;
    let no_picture = give(b"not a picture".to_vec()).await;
    assert_refused(&no_picture, bad, "invalid_request", "not a picture");
    let too_big = give(vec![0; 20 * 1024 * 1024 + 1]).await;
    assert_refused(&too_big, bad, "invalid_request", "too big");
    assert!(
        too_big.json()["message"]
            .as_str()
            .unwrap()
            .contains("20 MB")
    );
    let no_page = json!({ "book_id": second["id"], "page": 3 });
    let no_page = server.put(&series_cover, no_page).await;
    assert_refused(&no_page, StatusCode::NOT_FOUND, "not_found", "no page");

    // Someone else sees the covers and cannot choose them.
    server.add_user("Ben", &[library_id]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    assert_eq!(ben.get(&chosen_cover).await.status, StatusCode::OK);
    let forbidden = StatusCode::FORBIDDEN;
    let refused = ben.put(&series_cover, page).await;
    assert_refused(&refused, forbidden, "forbidden", "a member chooses");
    let refused = ben.delete(&series_cover).await;
    assert_refused(&refused, forbidden, "forbidden", "a member forgets");

    // Taken back, the series shows its first book's cover: the picture.
    let forgotten = server.delete(&series_cover).await;
    assert_eq!(forgotten.status, StatusCode::NO_CONTENT);
    let shown = series().await;
    assert_eq!(shown["cover_chosen"], false);
    assert_eq!(shown["cover"], with_picture["cover"]);
    assert_eq!(
        server.get(&chosen_cover).await.status,
        StatusCode::NOT_FOUND
    );

    let book_cover = format!("/api/v1/books/{}/cover", first["id"]);
    assert_eq!(
        server.delete(&book_cover).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(book().await["cover"], first["identity"]);
    assert_eq!(series().await["cover"], first["identity"]);
}

#[tokio::test]
async fn the_home_lists_follow_reading_and_adding() {
    let server = server().await;
    assert_eq!(
        server.get("/api/v1/continue-reading").await.json(),
        json!([])
    );
    assert_eq!(server.get("/api/v1/recently-added").await.json(), json!([]));

    fixtures::write_book(&server.comics.join("Saga/Saga 1.cbz"), 3, 1);
    fixtures::write_book(&server.comics.join("Saga/Saga 2.cbz"), 3, 2);
    fixtures::write_cbz(&server.comics.join("Broken.cbz"), &[]);
    server.add_library().await;

    let recent = server.get("/api/v1/recently-added").await.json();
    assert_eq!(recent.as_array().unwrap().len(), 2);
    let one = server.get("/api/v1/recently-added?limit=1").await.json();
    assert_eq!(one.as_array().unwrap().len(), 1);
    let none = server.get("/api/v1/recently-added?limit=many").await;
    assert_eq!(none.status, StatusCode::BAD_REQUEST);
    assert_eq!(none.json()["code"], "invalid_request");

    let id = recent[0]["id"].as_i64().unwrap();
    let address = format!("/api/v1/books/{id}/progress");
    server.put(&address, json!({ "page": 1 })).await;
    let reading = server.get("/api/v1/continue-reading").await.json();
    assert_eq!(reading.as_array().unwrap().len(), 1);
    assert_eq!(reading[0]["id"], id);
    assert_eq!(reading[0]["progress"]["page"], 1);

    assert_eq!(server.get("/api/v1/up-next").await.json(), json!([]));

    // A finished book is not one to continue: the one after it is next.
    server.put(&address, json!({ "page": 2 })).await;
    assert_eq!(
        server.get("/api/v1/continue-reading").await.json(),
        json!([])
    );
    let next = server.get("/api/v1/up-next").await.json();
    assert_eq!(next.as_array().unwrap().len(), 1);
    assert_ne!(next[0]["id"], id);
    assert_eq!(next[0]["series_id"], recent[0]["series_id"]);
    assert_eq!(next[0]["progress"], json!(null));

    let updated = server.get("/api/v1/recently-updated").await.json();
    assert_eq!(updated.as_array().unwrap().len(), 1);
    assert_eq!(updated[0]["name"], "Saga");
    assert_eq!(updated[0]["book_count"], 2);

    // The series can be taken off what is next.
    let off = format!("/api/v1/up-next/{}", next[0]["series_id"]);
    assert_eq!(server.delete(&off).await.status, StatusCode::NO_CONTENT);
    assert_eq!(server.get("/api/v1/up-next").await.json(), json!([]));
    let unknown = server.delete("/api/v1/up-next/9999").await;
    assert_refused(&unknown, StatusCode::NOT_FOUND, "not_found", "");
}

#[tokio::test]
async fn cbr_and_pdf_books_are_read_and_found_by_what_they_say() {
    let server = server().await;
    let scan = fixtures::jpeg_page(30, 45, 1);
    fixtures::write_cbr(
        &server.comics.join("Saga/Saga 1.cbr"),
        &[
            ("01.png", fixtures::page(40, 60, 1)),
            (
                "ComicInfo.xml",
                fixtures::comic_info(&[
                    ("Title", "The Long Night"),
                    ("Number", "1"),
                    ("Writer", "Élodie Durand"),
                    ("Year", "2019"),
                ]),
            ),
        ],
    );
    fixtures::write_pdf(
        &server.comics.join("Saga/Saga 2.pdf"),
        &[fixtures::PdfPage::Scan(scan.clone())],
    );
    let id = server.add_library().await;

    let series = server
        .get(&format!("/api/v1/libraries/{id}/series"))
        .await
        .json()["series"]
        .clone();
    let series_id = series[0]["id"].as_i64().unwrap();
    let books = server
        .get(&format!("/api/v1/series/{series_id}/books"))
        .await
        .json();
    assert_eq!(books[0]["name"], "Saga 1");
    assert_eq!(
        books[0]["info"],
        json!({
            "title": "The Long Night", "series": null, "number": "1", "volume": null,
            "year": 2019, "month": null, "day": null, "writer": "Élodie Durand",
            "penciller": null, "publisher": null, "summary": null, "language": null, "format": null,
        })
    );
    assert!(books[1]["info"].is_null());
    assert!(books[1]["problem"].is_null());
    assert!(books[1]["cover"].is_string());

    // The PDF's page is the picture it holds.
    let identity = books[1]["identity"].as_str().unwrap();
    let page = server.get(&format!("/api/v1/pages/{identity}/0")).await;
    assert_eq!(page.status, StatusCode::OK);
    assert_eq!(page.headers[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(page.body, scan);

    let found = server.get("/api/v1/search?q=elodie%20NIGHT").await.json();
    assert_eq!(found["series"], json!([]));
    assert_eq!(found["books"].as_array().unwrap().len(), 1);
    assert_eq!(found["books"][0], books[0]);

    let found = server.get("/api/v1/search?q=saga").await.json();
    assert_eq!(found["series"], series);
    assert_eq!(found["books"], books);
    assert_eq!(found["more_books"], false);

    for nothing in [
        "/api/v1/search",
        "/api/v1/search?q=",
        "/api/v1/search?q=zzz",
    ] {
        let found = server.get(nothing).await;
        assert_eq!(found.status, StatusCode::OK, "{nothing}");
        assert_eq!(found.json(), nothing_found(), "{nothing}");
    }

    // A member finds only what is in the libraries they were given.
    server.add_user("Ben", &[]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    for words in ["saga", "elodie"] {
        let found = ben.get(&format!("/api/v1/search?q={words}")).await.json();
        assert_eq!(found, nothing_found(), "{words}");
    }
}

/// What a search that finds nothing answers.
fn nothing_found() -> Value {
    json!({
        "series": [], "more_series": false, "books": [], "more_books": false,
        "lists": [], "more_lists": false, "collections": [], "more_collections": false,
        "people": [], "more_people": false,
    })
}

impl Server {
    async fn patch(&self, path: &str, body: Value) -> Answer {
        self.send_json(Method::PATCH, path, body).await
    }

    async fn delete(&self, path: &str) -> Answer {
        self.call(Method::DELETE, path).await
    }

    /// Adds a user as the owner, and returns their id.
    async fn add_user(&self, name: &str, libraries: &[i64]) -> i64 {
        let new = json!({ "name": name, "password": PASSWORD, "libraries": libraries });
        let added = self.post("/api/v1/users", new).await;
        assert_eq!(added.status, StatusCode::CREATED, "{name}");
        added.json()["id"].as_i64().unwrap()
    }

    /// Makes a token for whoever is asking, and returns its id and secret.
    async fn make_token(&self, name: &str) -> (i64, String) {
        let made = self.post("/api/v1/tokens", json!({ "name": name })).await;
        assert_eq!(made.status, StatusCode::CREATED);
        let made = made.json();
        (
            made["token"]["id"].as_i64().unwrap(),
            made["secret"].as_str().unwrap().to_owned(),
        )
    }

    /// The ids of a library's series, in name order.
    async fn series_ids(&self, library_id: i64) -> Vec<i64> {
        let series = self
            .get(&format!("/api/v1/libraries/{library_id}/series"))
            .await
            .json()["series"]
            .clone();
        let series = series.as_array().unwrap();
        series
            .iter()
            .map(|one| one["id"].as_i64().unwrap())
            .collect()
    }

    /// Adds a second folder of comics as a library, and returns its id.
    async fn add_second_library(&self, name: &str) -> i64 {
        let folder = self.comics.parent().unwrap().join(name);
        fixtures::write_book(&folder.join(format!("{name} 1.cbz")), 2, 77);
        self.add_library_at(&folder).await
    }
}

fn assert_refused(answer: &Answer, status: StatusCode, code: &str, what: &str) {
    assert_eq!(answer.status, status, "{what}");
    assert_eq!(answer.json()["code"], code, "{what}");
    assert!(
        answer.json()["message"]
            .as_str()
            .is_some_and(|text| !text.is_empty()),
        "{what}"
    );
}

#[tokio::test]
async fn a_new_server_waits_for_its_owner() {
    let server = new_server().await;
    let health = server.get("/api/v1/health").await.json();
    assert_eq!(health["needs_setup"], true);

    // Nobody can be signed in yet.
    let closed = server.get("/api/v1/libraries").await;
    assert_refused(
        &closed,
        StatusCode::UNAUTHORIZED,
        "unauthorized",
        "libraries",
    );
    let nobody = json!({ "name": OWNER, "password": PASSWORD });
    let sign_in = server.post("/api/v1/sign-in", nobody.clone()).await;
    assert_refused(
        &sign_in,
        StatusCode::UNAUTHORIZED,
        "invalid_credentials",
        "sign-in",
    );

    for bad in [
        json!({ "name": OWNER, "password": "short" }),
        json!({ "name": "  ", "password": PASSWORD }),
        json!({ "name": OWNER }),
    ] {
        let refused = server.post("/api/v1/setup", bad.clone()).await;
        assert!(refused.status.is_client_error(), "{bad}");
        assert_eq!(refused.json()["code"], "invalid_request", "{bad}");
    }
    assert_eq!(
        server.get("/api/v1/health").await.json()["needs_setup"],
        true
    );

    let set_up = server.post("/api/v1/setup", nobody.clone()).await;
    assert_eq!(set_up.status, StatusCode::CREATED);
    let owner = set_up.json();
    assert_eq!(owner["name"], OWNER);
    assert_eq!(owner["role"], "owner");
    assert_eq!(owner["libraries"], json!([]));
    // Neither the password nor anything made from it comes back.
    assert!(!String::from_utf8_lossy(&set_up.body).contains("password"));
    let cookie = set_up.headers[header::SET_COOKIE].to_str().unwrap();
    for part in ["HttpOnly", "SameSite=Lax", "Path=/api"] {
        assert!(cookie.contains(part), "{cookie}");
    }

    let as_owner = server.with_session_of(&set_up);
    assert_eq!(as_owner.get("/api/v1/me").await.json(), owner);
    assert_eq!(as_owner.get("/api/v1/libraries").await.json(), json!([]));
    assert_eq!(
        server.get("/api/v1/health").await.json()["needs_setup"],
        false
    );

    // The server has its owner; there is no second one.
    let again = json!({ "name": "Eve", "password": PASSWORD });
    let again = server.post("/api/v1/setup", again).await;
    assert_refused(&again, StatusCode::CONFLICT, "conflict", "second setup");
    let eve = json!({ "name": "Eve", "password": PASSWORD });
    let eve = server.post("/api/v1/sign-in", eve).await;
    assert_eq!(eve.status, StatusCode::UNAUTHORIZED);
}

/// What the API description says a route asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asks {
    Nothing,
    SessionOrToken,
    Session,
}

/// Every route the API describes, as (method, path with ids filled in, what
/// it asks for).
fn described_routes() -> Vec<(Method, String, Asks)> {
    let description = serde_json::to_value(api::description()).unwrap();
    let by_default = &description["security"];
    assert_eq!(by_default, &json!([{ "token": [] }, { "session": [] }]));
    let mut routes = Vec::new();
    for (path, methods) in description["paths"].as_object().unwrap() {
        // Every `{name}` in a path stands for an id.
        let mut filled = String::new();
        for (index, part) in path.split(['{', '}']).enumerate() {
            filled.push_str(if index % 2 == 0 { part } else { "1" });
        }
        for (method, operation) in methods.as_object().unwrap() {
            let asks = match operation.get("security") {
                None => Asks::SessionOrToken,
                Some(nothing) if nothing == &json!([{}]) => Asks::Nothing,
                Some(session) if session == &json!([{ "session": [] }]) => Asks::Session,
                Some(other) => panic!("{method} {path} asks for {other}"),
            };
            let method = Method::from_bytes(method.to_uppercase().as_bytes()).unwrap();
            routes.push((method, filled.clone(), asks));
        }
    }
    routes
}

#[tokio::test]
async fn only_health_setup_and_signing_in_are_open() {
    let nobody = server().await.signed_out();
    let mut routes = described_routes();
    assert!(routes.len() > 25, "{}", routes.len());
    routes.push((
        Method::GET,
        "/api/v1/openapi.json".to_owned(),
        Asks::SessionOrToken,
    ));

    // Opening a route to everyone is a decision, so the list is written out.
    let open: Vec<&str> = routes
        .iter()
        .filter(|(_, _, asks)| *asks == Asks::Nothing)
        .map(|(_, path, _)| path.as_str())
        .collect();
    assert_eq!(open, ["/api/v1/health", "/api/v1/setup", "/api/v1/sign-in"]);

    // Taken from the description, so a route added later is asked too, and
    // the description is held to what the server does.
    for (method, path, asks) in &routes {
        let what = format!("{method} {path}");
        let answer = nobody.send_json(method.clone(), path, json!({})).await;
        if *asks == Asks::Nothing {
            assert_ne!(answer.status, StatusCode::UNAUTHORIZED, "{what}");
            continue;
        }
        assert_refused(&answer, StatusCode::UNAUTHORIZED, "unauthorized", &what);
        assert_eq!(answer.headers[header::WWW_AUTHENTICATE], "Bearer", "{what}");

        // What is shown has to be good, too.
        for who in [
            Who::Session("binge_session=made-up".to_owned()),
            Who::Token("binge_made_up".to_owned()),
        ] {
            let stranger = nobody.asked_as(who);
            let answer = stranger.send_json(method.clone(), path, json!({})).await;
            assert_refused(&answer, StatusCode::UNAUTHORIZED, "unauthorized", &what);
        }
    }
    // The web UI itself is open: it is where signing in happens.
    assert_eq!(nobody.get("/").await.status, StatusCode::OK);
}

#[tokio::test]
async fn signing_in_takes_the_right_password_and_signing_out_ends_it() {
    let server = server().await;
    for (name, password) in [(OWNER, "wrong horse"), ("Nobody", PASSWORD), (OWNER, "")] {
        let shown = json!({ "name": name, "password": password });
        let refused = server.signed_out().post("/api/v1/sign-in", shown).await;
        assert_refused(
            &refused,
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            name,
        );
        assert!(!refused.headers.contains_key(header::SET_COOKIE));
    }

    let here = server.signed_in_as("ana", PASSWORD).await;
    assert_eq!(here.get("/api/v1/me").await.json()["name"], OWNER);

    let out = here.call(Method::POST, "/api/v1/sign-out").await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    let cleared = out.headers[header::SET_COOKIE].to_str().unwrap();
    assert!(cleared.starts_with("binge_session=;"), "{cleared}");
    assert!(cleared.contains("Max-Age=0"), "{cleared}");
    // The session is over on the server, not only forgotten by the browser.
    let after = here.get("/api/v1/me").await;
    assert_refused(&after, StatusCode::UNAUTHORIZED, "unauthorized", "after");
    // The session from the setup is another one, and goes on.
    assert_eq!(server.get("/api/v1/me").await.status, StatusCode::OK);
}

#[tokio::test]
async fn a_token_works_until_it_is_revoked() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("One Shot.cbz"), 2, 1);
    let library = server.add_library().await;

    let no_name = server.post("/api/v1/tokens", json!({ "name": " " })).await;
    assert_refused(&no_name, StatusCode::BAD_REQUEST, "invalid_request", "name");
    let (id, secret) = server.make_token("iPad").await;
    let listed = server.get("/api/v1/tokens").await;
    assert_eq!(listed.json()[0]["id"], id);
    assert_eq!(listed.json()[0]["name"], "iPad");
    assert!(listed.json()[0]["last_used_at"].is_null());
    // The secret is told once, when the token is made.
    assert!(!String::from_utf8_lossy(&listed.body).contains(&secret));

    // An app reads with it, as the user it was made by.
    let app = server.with_token(&secret);
    assert_eq!(app.get("/api/v1/me").await.json()["name"], OWNER);
    let book = app.first_book(library).await;
    let identity = book["identity"].as_str().unwrap();
    let page = app.get(&format!("/api/v1/pages/{identity}/0")).await;
    assert_eq!(page.status, StatusCode::OK);
    let saved = app
        .put(
            &format!("/api/v1/books/{}/progress", book["id"]),
            json!({ "page": 1 }),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK);
    assert!(server.get("/api/v1/tokens").await.json()[0]["last_used_at"].is_i64());

    // It does not manage the account it came from, or any other: every
    // route the description keeps for sessions refuses it.
    let routes = described_routes();
    let in_person = |asks| routes.iter().filter(move |route| route.2 == asks);
    assert_eq!(in_person(Asks::Session).count(), 8);
    for (method, path, _) in in_person(Asks::Session) {
        let what = format!("{method} {path}");
        let refused = app.send_json(method.clone(), path, json!({})).await;
        assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", &what);
        assert!(
            refused.json()["message"]
                .as_str()
                .unwrap()
                .contains("token")
        );
    }
    // Reading is not refused for being asked with a token.
    for (method, path, _) in in_person(Asks::SessionOrToken) {
        if method == Method::GET {
            let answer = app.get(path).await;
            assert_ne!(answer.status, StatusCode::FORBIDDEN, "{path}");
            assert_ne!(answer.status, StatusCode::UNAUTHORIZED, "{path}");
        }
    }

    // A token is the whole answer to who is asking: a bad one is not made
    // good by a session that comes with it.
    let both = Request::get("/api/v1/me")
        .header(header::AUTHORIZATION, "Bearer binge_made_up")
        .body(Body::empty())
        .unwrap();
    assert_eq!(server.send(both).await.status, StatusCode::UNAUTHORIZED);
    for shown in ["binge_made_up", "", &secret[..secret.len() - 1]] {
        let answer = server.with_token(shown).get("/api/v1/me").await;
        assert_refused(&answer, StatusCode::UNAUTHORIZED, "unauthorized", shown);
    }

    let revoked = server.delete(&format!("/api/v1/tokens/{id}")).await;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT);
    for path in [
        "/api/v1/me".to_owned(),
        format!("/api/v1/pages/{identity}/0"),
    ] {
        let after = app.get(&path).await;
        assert_refused(&after, StatusCode::UNAUTHORIZED, "unauthorized", &path);
    }
    let twice = server.delete(&format!("/api/v1/tokens/{id}")).await;
    assert_refused(&twice, StatusCode::NOT_FOUND, "not_found", "twice");
    assert_eq!(server.get("/api/v1/tokens").await.json(), json!([]));
}

#[tokio::test]
async fn a_member_reads_only_the_libraries_they_were_given() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("Saga/Saga 1.cbz"), 2, 1);
    let comics = server.add_library().await;
    let manga = server.add_second_library("Manga").await;
    server.add_user("Ben", &[manga]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;

    let me = ben.get("/api/v1/me").await.json();
    assert_eq!(me["role"], "member");
    assert_eq!(me["libraries"], json!([manga]));

    // The owner is told where a library is on the disk; a member is not.
    let owners = server.get("/api/v1/libraries").await.json();
    assert_eq!(owners.as_array().unwrap().len(), 2);
    assert!(owners[0]["path"].is_string());
    let bens = ben.get("/api/v1/libraries").await.json();
    assert_eq!(bens.as_array().unwrap().len(), 1);
    assert_eq!(bens[0]["id"], manga);
    assert!(bens[0]["path"].is_null());
    let recent = ben.get("/api/v1/recently-added").await.json();
    assert_eq!(recent.as_array().unwrap().len(), 1);
    assert_eq!(recent[0]["library_id"], manga);

    // What he was given, he reads.
    let his = ben.first_book(manga).await;
    let his_identity = his["identity"].as_str().unwrap();
    for path in [
        format!("/api/v1/books/{}/pages", his["id"]),
        format!("/api/v1/pages/{his_identity}/0"),
        format!("/api/v1/covers/{}", his["cover"].as_str().unwrap()),
    ] {
        assert_eq!(ben.get(&path).await.status, StatusCode::OK, "{path}");
    }

    // The other library is not there for him, by any address.
    let series = server
        .get(&format!("/api/v1/libraries/{comics}/series"))
        .await
        .json()["series"][0]["id"]
        .as_i64()
        .unwrap();
    let book = server
        .get(&format!("/api/v1/series/{series}/books"))
        .await
        .json()[0]
        .clone();
    let (book_id, identity) = (
        book["id"].as_i64().unwrap(),
        book["identity"].as_str().unwrap(),
    );
    for path in [
        format!("/api/v1/libraries/{comics}"),
        format!("/api/v1/libraries/{comics}/series"),
        format!("/api/v1/series/{series}"),
        format!("/api/v1/series/{series}/books"),
        format!("/api/v1/series/{series}/overview"),
        format!("/api/v1/books/{book_id}"),
        format!("/api/v1/books/{book_id}/pages"),
        format!("/api/v1/pages/{identity}/0"),
        format!("/api/v1/covers/{}", book["cover"].as_str().unwrap()),
    ] {
        assert_eq!(server.get(&path).await.status, StatusCode::OK, "{path}");
        let hidden = ben.get(&path).await;
        assert_refused(&hidden, StatusCode::NOT_FOUND, "not_found", &path);
    }
    let progress = format!("/api/v1/books/{book_id}/progress");
    let hidden = ben.put(&progress, json!({ "page": 0 })).await;
    assert_refused(&hidden, StatusCode::NOT_FOUND, "not_found", &progress);
    // Holding the image's entity tag does not get him the page either.
    let tagged = Request::get(format!("/api/v1/pages/{identity}/0"))
        .header(header::IF_NONE_MATCH, format!("\"{identity}-0\""));
    let tagged = ben.send(tagged.body(Body::empty()).unwrap()).await;
    assert_eq!(tagged.status, StatusCode::NOT_FOUND);

    // Libraries, how they are laid out, and users are the owner's to manage.
    for (method, path) in [
        (Method::POST, "/api/v1/libraries".to_owned()),
        (Method::POST, format!("/api/v1/libraries/{manga}/scan")),
        (Method::DELETE, format!("/api/v1/libraries/{manga}")),
        (Method::POST, format!("/api/v1/libraries/{manga}/series")),
        (Method::PATCH, "/api/v1/series/1".to_owned()),
        (Method::DELETE, "/api/v1/series/1".to_owned()),
        (Method::POST, "/api/v1/series/1/groups".to_owned()),
        (Method::PATCH, "/api/v1/groups/1".to_owned()),
        (Method::DELETE, "/api/v1/groups/1".to_owned()),
        (Method::PUT, "/api/v1/books/1/place".to_owned()),
        (Method::GET, "/api/v1/users".to_owned()),
        (Method::POST, "/api/v1/users".to_owned()),
        (Method::PATCH, "/api/v1/users/1".to_owned()),
        (Method::DELETE, "/api/v1/users/1".to_owned()),
    ] {
        let what = format!("{method} {path}");
        let body = json!({
            "path": server.comics, "name": "x", "password": PASSWORD,
            "kind": "one_shot", "series_id": 1,
        });
        let refused = ben.send_json(method, &path, body).await;
        assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", &what);
    }
    assert_eq!(
        server
            .get("/api/v1/libraries")
            .await
            .json()
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn everyone_has_their_own_progress() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("One Shot.cbz"), 4, 1);
    let library = server.add_library().await;
    server.add_user("Ben", &[library]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    let id = server.first_book(library).await["id"].as_i64().unwrap();
    let progress = format!("/api/v1/books/{id}/progress");

    server.put(&progress, json!({ "page": 1 })).await;
    assert!(ben.first_book(library).await["progress"].is_null());
    assert_eq!(ben.get("/api/v1/continue-reading").await.json(), json!([]));

    ben.put(&progress, json!({ "page": 3 })).await;
    let bens = ben.get(&format!("/api/v1/books/{id}")).await.json();
    assert_eq!(bens["progress"]["page"], 3);
    assert_eq!(bens["progress"]["completed"], true);
    let owners = server.get(&format!("/api/v1/books/{id}")).await.json();
    assert_eq!(owners["progress"]["page"], 1);
    let reading = server.get("/api/v1/continue-reading").await.json();
    assert_eq!(reading[0]["id"], id);

    // A token reads and saves as the user it was made by.
    let (_, secret) = ben.make_token("Ben's phone").await;
    let phone = server.with_token(&secret);
    let seen = phone.get(&format!("/api/v1/books/{id}")).await.json();
    assert_eq!(seen["progress"]["page"], 3);
}

#[tokio::test]
async fn the_owner_manages_the_other_users() {
    let server = server().await;
    let library = server.add_library().await;
    let id = server.add_user("Ben", &[]).await;
    let ben = server.signed_in_as("ben", PASSWORD).await;
    let (_, secret) = ben.make_token("phone").await;
    assert_eq!(ben.get("/api/v1/libraries").await.json(), json!([]));

    let users = server.get("/api/v1/users").await.json();
    let names: Vec<_> = users
        .as_array()
        .unwrap()
        .iter()
        .map(|user| {
            (
                user["name"].as_str().unwrap(),
                user["role"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(names, [(OWNER, "owner"), ("Ben", "member")]);
    let owner_id = users[0]["id"].as_i64().unwrap();

    for (new, status, code) in [
        (
            json!({ "name": "BEN", "password": PASSWORD }),
            StatusCode::CONFLICT,
            "conflict",
        ),
        (
            json!({ "name": "Cy", "password": "short" }),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        ),
        (
            json!({ "name": "Cy", "password": PASSWORD, "libraries": [99] }),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        ),
    ] {
        let refused = server.post("/api/v1/users", new.clone()).await;
        assert_refused(&refused, status, code, &new["name"].to_string());
    }

    // Other libraries: his session goes on and sees them at once.
    let address = format!("/api/v1/users/{id}");
    let changed = server
        .patch(&address, json!({ "libraries": [library] }))
        .await;
    assert_eq!(changed.status, StatusCode::OK);
    assert_eq!(changed.json()["libraries"], json!([library]));
    assert_eq!(ben.get("/api/v1/libraries").await.json()[0]["id"], library);

    // A new password: his session ends, his token stays.
    let changed = server
        .patch(&address, json!({ "password": "another horse" }))
        .await;
    assert_eq!(changed.status, StatusCode::OK);
    assert_eq!(changed.json()["libraries"], json!([library]));
    assert_eq!(ben.get("/api/v1/me").await.status, StatusCode::UNAUTHORIZED);
    let phone = server.with_token(&secret);
    assert_eq!(phone.get("/api/v1/me").await.status, StatusCode::OK);
    server.signed_in_as("Ben", "another horse").await;

    let owner = format!("/api/v1/users/{owner_id}");
    for refused in [
        server.patch(&owner, json!({ "libraries": [] })).await,
        server.delete(&owner).await,
    ] {
        assert_refused(
            &refused,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "owner",
        );
    }
    let nobody = server.delete("/api/v1/users/99").await;
    assert_refused(&nobody, StatusCode::NOT_FOUND, "not_found", "nobody");

    assert_eq!(server.delete(&address).await.status, StatusCode::NO_CONTENT);
    assert_eq!(
        phone.get("/api/v1/me").await.status,
        StatusCode::UNAUTHORIZED
    );
    let gone = json!({ "name": "Ben", "password": "another horse" });
    let gone = server.signed_out().post("/api/v1/sign-in", gone).await;
    assert_eq!(gone.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        server
            .get("/api/v1/users")
            .await
            .json()
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn changing_a_password_takes_the_current_one() {
    let server = server().await;
    let elsewhere = server.signed_in_as(OWNER, PASSWORD).await;
    let address = "/api/v1/me/password";

    for (change, what) in [
        (
            json!({ "current": "wrong horse", "new": "another horse" }),
            "wrong",
        ),
        (json!({ "current": PASSWORD, "new": "short" }), "short"),
    ] {
        let refused = server.put(address, change).await;
        assert_refused(&refused, StatusCode::BAD_REQUEST, "invalid_request", what);
    }
    assert_eq!(elsewhere.get("/api/v1/me").await.status, StatusCode::OK);

    let change = json!({ "current": PASSWORD, "new": "another horse" });
    assert_eq!(
        server.put(address, change).await.status,
        StatusCode::NO_CONTENT
    );
    // This browser stays signed in; the other one does not.
    assert_eq!(server.get("/api/v1/me").await.status, StatusCode::OK);
    assert_eq!(
        elsewhere.get("/api/v1/me").await.status,
        StatusCode::UNAUTHORIZED
    );
    let old = json!({ "name": OWNER, "password": PASSWORD });
    let old = server.signed_out().post("/api/v1/sign-in", old).await;
    assert_eq!(old.status, StatusCode::UNAUTHORIZED);
    server.signed_in_as(OWNER, "another horse").await;
}

#[tokio::test]
async fn a_change_asked_for_by_another_page_is_refused() {
    let server = server().await;
    let (_, secret) = server.make_token("script").await;
    let sign_out = |headers: &[(&'static str, &'static str)]| {
        let mut request = Request::post("/api/v1/sign-out").header(header::HOST, "nas.local:8686");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        request.body(Body::empty()).unwrap()
    };

    for from in [
        [("origin", "http://evil.example")],
        [("origin", "http://nas.local:9000")],
        [("sec-fetch-site", "cross-site")],
        [("sec-fetch-site", "same-site")],
    ] {
        let refused = server.send(sign_out(&from)).await;
        assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", from[0].1);
    }
    // Nothing happened: the session is still there.
    assert_eq!(server.get("/api/v1/me").await.status, StatusCode::OK);

    // Signing in from another page is refused too, before any password is
    // looked at.
    let sign_in = Request::post("/api/v1/sign-in")
        .header(header::HOST, "nas.local:8686")
        .header(header::ORIGIN, "http://evil.example")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "name": OWNER, "password": PASSWORD }).to_string(),
        ))
        .unwrap();
    let refused = server.signed_out().send(sign_in).await;
    assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", "sign-in");

    // A request with a token has no cookie to misuse, wherever it is from.
    let app = server.with_token(&secret);
    let from_app = app
        .send(sign_out(&[("origin", "http://evil.example")]))
        .await;
    assert_eq!(from_app.status, StatusCode::NO_CONTENT);

    // What a proxy's own sign-in adds to every request is not a token: the
    // request still stands on its cookie, and is refused from another page.
    const PROXY: (&str, &str) = ("authorization", "Basic made-up");
    let me = Request::get("/api/v1/me")
        .header(header::HOST, "nas.local:8686")
        .header(PROXY.0, PROXY.1)
        .body(Body::empty())
        .unwrap();
    assert_eq!(server.send(me).await.status, StatusCode::OK);
    let refused = server
        .send(sign_out(&[PROXY, ("origin", "http://evil.example")]))
        .await;
    assert_refused(
        &refused,
        StatusCode::FORBIDDEN,
        "forbidden",
        "behind a proxy",
    );

    // The server's own page.
    let own = server
        .send(sign_out(&[("origin", "http://nas.local:8686")]))
        .await;
    assert_eq!(own.status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn nothing_of_the_server_shows_in_a_frame() {
    let server = server().await;
    for path in ["/", "/settings", "/api/v1/health", "/api/v1/me"] {
        let answer = server.get(path).await;
        assert_eq!(answer.headers[header::X_FRAME_OPTIONS], "DENY", "{path}");
        assert_eq!(
            answer.headers[header::CONTENT_SECURITY_POLICY],
            "frame-ancestors 'none'",
            "{path}"
        );
    }
}

impl Server {
    /// Scans a library again and waits for it.
    async fn rescan(&self, library_id: i64) {
        let scan = format!("/api/v1/libraries/{library_id}/scan");
        let started = self.call(Method::POST, &scan).await;
        assert_eq!(started.status, StatusCode::ACCEPTED);
        self.catalog.scan_finished(library_id).await;
    }

    /// The book of that file name, among the books of a library's series.
    async fn book_named(&self, library_id: i64, name: &str) -> Value {
        let series = self
            .get(&format!("/api/v1/libraries/{library_id}/series"))
            .await
            .json()["series"]
            .clone();
        for series in series.as_array().unwrap() {
            let books = format!("/api/v1/series/{}/books", series["id"]);
            let books = self.get(&books).await.json();
            if let Some(book) = books.as_array().unwrap().iter().find(|b| b["name"] == name) {
                return book.clone();
            }
        }
        panic!("no book is called {name}");
    }

    /// The names of the books a path lists.
    async fn names_at(&self, path: &str) -> Vec<String> {
        let books = self.get(path).await.json();
        let names = books.as_array().unwrap().iter();
        names
            .map(|b| b["name"].as_str().unwrap().to_owned())
            .collect()
    }
}

#[tokio::test]
async fn a_reading_list_holds_books_in_an_order_across_series() {
    let server = server().await;
    for (seed, file) in [
        "Patrol/Patrol 001",
        "Patrol/Patrol 002",
        "Saga/Saga 001",
        "Saga/Saga 002",
    ]
    .iter()
    .enumerate()
    {
        fixtures::write_book(
            &server.comics.join(format!("{file}.cbz")),
            2,
            seed as u8 + 1,
        );
    }
    let comics = server.add_library().await;
    let manga = server.add_second_library("Manga").await;
    let [patrol_1, patrol_2, saga_1, saga_2] = ["Patrol 001", "Patrol 002", "Saga 001", "Saga 002"]
        .map(async |name| server.book_named(comics, name).await);
    let (patrol_1, patrol_2, saga_1, saga_2) =
        (patrol_1.await, patrol_2.await, saga_1.await, saga_2.await);

    // Made with a book, then a whole series in the order it is read.
    let new = json!({
        "name": "Crossover",
        "summary": "  Two worlds meet.  ",
        "books": [saga_1["id"], patrol_1["id"]],
        "series": [patrol_1["series_id"]],
    });
    let made = server.post("/api/v1/reading-lists", new).await;
    assert_eq!(made.status, StatusCode::CREATED);
    let list = made.json();
    assert_eq!(list["name"], "Crossover");
    assert_eq!(list["summary"], "Two worlds meet.");
    assert_eq!(list["book_count"], 3);
    assert_eq!(list["unread_count"], 3);
    assert_eq!(list["cover"], saga_1["cover"]);
    assert_eq!(list["cover_chosen"], false);
    assert_eq!(list["covers"].as_array().unwrap().len(), 3);
    let at = format!("/api/v1/reading-lists/{}", list["id"]);
    let books = format!("{at}/books");
    assert_eq!(
        server.names_at(&books).await,
        ["Saga 001", "Patrol 001", "Patrol 002"]
    );
    let all = server.get("/api/v1/reading-lists").await.json();
    assert_eq!(all.as_array().unwrap().len(), 1);

    // A name written another way is the same name, and nothing is made of a
    // book that is not there.
    let again = server
        .post("/api/v1/reading-lists", json!({ "name": "cross-over" }))
        .await;
    assert_refused(
        &again,
        StatusCode::CONFLICT,
        "conflict",
        "the name is taken",
    );
    assert_eq!(again.json()["conflicts_with"], list["id"]);
    let missing = json!({ "name": "Another", "books": [9999] });
    let refused = server.post("/api/v1/reading-lists", missing).await;
    assert_refused(
        &refused,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "no such book",
    );
    let refused = server.post(&books, json!({ "series": [9999] })).await;
    assert_refused(
        &refused,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "no such series",
    );
    let all = server.get("/api/v1/reading-lists").await.json();
    assert_eq!(all.as_array().unwrap().len(), 1);

    // Added at the end; a book already there stays where it is.
    let more = json!({ "books": [saga_2["id"], saga_1["id"]] });
    let added = server.post(&books, more).await;
    assert_eq!(added.json(), json!({ "books": 1 }));
    assert_eq!(
        server.names_at(&books).await,
        ["Saga 001", "Patrol 001", "Patrol 002", "Saga 002"]
    );

    // Moved, to a position and past the end.
    let place_of = |book: &Value| format!("{books}/{}/position", book["id"]);
    let moved = server
        .put(&place_of(&saga_2), json!({ "position": 1 }))
        .await;
    assert_eq!(moved.status, StatusCode::NO_CONTENT);
    let moved = server
        .put(&place_of(&patrol_1), json!({ "position": 99 }))
        .await;
    assert_eq!(moved.status, StatusCode::NO_CONTENT);
    assert_eq!(
        server.names_at(&books).await,
        ["Saga 001", "Saga 002", "Patrol 002", "Patrol 001"]
    );
    let below = server
        .put(&place_of(&saga_2), json!({ "position": -1 }))
        .await;
    assert_refused(
        &below,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "below 0",
    );

    // Taken out, once.
    let patrol_2_there = format!("{books}/{}", patrol_2["id"]);
    assert_eq!(
        server.delete(&patrol_2_there).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        server.delete(&patrol_2_there).await.status,
        StatusCode::NOT_FOUND
    );
    let not_in_it = server
        .put(&place_of(&patrol_2), json!({ "position": 0 }))
        .await;
    assert_eq!(not_in_it.status, StatusCode::NOT_FOUND);

    // What to read next follows the list, not the series.
    let overview = server.get(&at).await.json();
    assert_eq!(overview["list"]["book_count"], 3);
    assert_eq!(overview["read"]["next"]["name"], "Saga 001");
    for book in [&saga_1, &saga_2] {
        let progress = format!("/api/v1/books/{}/progress", book["id"]);
        assert_eq!(
            server.put(&progress, json!({ "page": 1 })).await.status,
            StatusCode::OK
        );
    }
    let overview = server.get(&at).await.json();
    assert_eq!(overview["read"]["next"]["name"], "Patrol 001");
    assert_eq!(overview["list"]["unread_count"], 1);

    // A list is marked as a series is.
    let whole = json!({ "read": true, "lists": [list["id"]] });
    let marked = server.post("/api/v1/read-marks", whole).await;
    assert_eq!(marked.json(), json!({ "books": 1 }));
    assert_eq!(server.get(&at).await.json()["list"]["unread_count"], 0);

    // A book says which lists it is in.
    let of_book = |book: &Value| format!("/api/v1/books/{}/reading-lists", book["id"]);
    let in_lists = server.get(&of_book(&saga_1)).await.json();
    assert_eq!(in_lists[0]["id"], list["id"]);
    let in_none = server.get(&of_book(&patrol_2)).await.json();
    assert_eq!(in_none, json!([]));

    // A book stays in the list while its file is away, and is back in its
    // place when the file is.
    let file = server.comics.join("Saga/Saga 002.cbz");
    let kept = file.with_extension("away");
    std::fs::rename(&file, &kept).unwrap();
    server.rescan(comics).await;
    assert_eq!(server.names_at(&books).await, ["Saga 001", "Patrol 001"]);
    std::fs::rename(&kept, server.comics.join("Saga/Saga two.cbz")).unwrap();
    server.rescan(comics).await;
    assert_eq!(
        server.names_at(&books).await,
        ["Saga 001", "Saga two", "Patrol 001"]
    );

    // Shared with everyone, a member sees it by the books he may read, and
    // changes nothing.
    server.add_user("Ben", &[manga]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    let shared = server
        .put(&format!("{at}/sharing"), json!({ "everyone": true }))
        .await;
    assert_eq!(shared.status, StatusCode::OK);
    assert_eq!(shared.json()["shared_with_all"], true);
    assert_eq!(ben.get("/api/v1/reading-lists").await.json(), json!([]));
    assert_eq!(ben.get(&at).await.status, StatusCode::NOT_FOUND);
    assert_eq!(ben.get(&books).await.status, StatusCode::NOT_FOUND);
    let his = ben.first_book(manga).await;
    let added = server.post(&books, json!({ "books": [his["id"]] })).await;
    assert_eq!(added.json(), json!({ "books": 1 }));
    let bens = ben.get("/api/v1/reading-lists").await.json();
    assert_eq!(bens[0]["book_count"], 1);
    assert_eq!(bens[0]["cover"], his["cover"]);
    assert_eq!(ben.names_at(&books).await, ["Manga 1"]);
    assert_eq!(bens[0]["mine"], false);
    assert_eq!(bens[0]["made_by"]["name"], OWNER);
    for refused in [
        ben.post(&books, json!({ "books": [his["id"]] })).await,
        ben.delete(&format!("{books}/{}", his["id"])).await,
        ben.put(&place_of(&his), json!({ "position": 0 })).await,
        ben.patch(&at, json!({ "name": "Mine" })).await,
        ben.put(&format!("{at}/sharing"), json!({})).await,
        ben.delete(&at).await,
    ] {
        assert_refused(
            &refused,
            StatusCode::FORBIDDEN,
            "forbidden",
            "someone else's list",
        );
    }

    // What it is about is what the list says, not what a book of it does.
    assert_eq!(
        server.get(&at).await.json()["summary"],
        json!({ "text": "Two worlds meet.", "book": null })
    );

    // Another name, and no summary any more.
    let renamed = server.patch(&at, json!({ "name": "Two Worlds" })).await;
    assert_eq!(renamed.status, StatusCode::OK);
    assert_eq!(renamed.json()["name"], "Two Worlds");
    assert!(renamed.json()["summary"].is_null());
    assert!(server.get(&at).await.json()["summary"].is_null());
    // A list that is not there is not there, whatever name is asked for it.
    let missing = server
        .patch(
            "/api/v1/reading-lists/9999",
            json!({ "name": "Two Worlds" }),
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    let nameless = server.patch(&at, json!({ "name": "  " })).await;
    assert_refused(
        &nameless,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "no name",
    );

    // A cover of its own, which goes with the list.
    let cover = json!({ "book_id": patrol_1["id"], "page": 1 });
    let chosen = server.put(&format!("{at}/cover"), cover).await;
    assert_eq!(chosen.status, StatusCode::NO_CONTENT);
    let shown = server.get(&at).await.json()["list"].clone();
    assert_eq!(shown["cover_chosen"], true);
    // The chosen one stands alone.
    assert_eq!(shown["covers"], json!([]));
    let image = format!("/api/v1/covers/{}", shown["cover"].as_str().unwrap());
    assert_eq!(server.get(&image).await.status, StatusCode::OK);
    assert_eq!(ben.get(&image).await.status, StatusCode::OK);
    assert_eq!(server.delete(&at).await.status, StatusCode::NO_CONTENT);
    assert_eq!(server.get(&image).await.status, StatusCode::NOT_FOUND);
    assert_eq!(server.get(&at).await.status, StatusCode::NOT_FOUND);
    assert_eq!(server.delete(&at).await.status, StatusCode::NOT_FOUND);
    // The books are as they were.
    assert_eq!(server.get(&of_book(&saga_1)).await.json(), json!([]));
}

#[tokio::test]
async fn a_collection_holds_series_and_is_its_makers_to_share() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("Saga/Saga 001.cbz"), 2, 1);
    fixtures::write_book(&server.comics.join("Paper Girls/Paper Girls 001.cbz"), 2, 2);
    let comics = server.add_library().await;
    let manga = server.add_second_library("Manga").await;
    let ben_id = server.add_user("Ben", &[comics, manga]).await;
    let cleo_id = server.add_user("Cleo", &[manga]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    let cleo = server.signed_in_as("Cleo", PASSWORD).await;
    let [paper, saga] = server.series_ids(comics).await[..] else {
        panic!("two series");
    };
    let manga_series = server.series_ids(manga).await[0];
    let collections = "/api/v1/collections";

    // Anyone makes a collection, and it is theirs alone. A series named
    // twice is in it once, and its series are shown by name.
    let new =
        json!({ "name": "Vaughan", "summary": " One writer ", "series": [saga, saga, paper] });
    let made = ben.post(collections, new).await;
    assert_eq!(made.status, StatusCode::CREATED);
    let collection = made.json();
    assert_eq!(collection["summary"], "One writer");
    assert_eq!(collection["series_count"], 2);
    assert_eq!(collection["book_count"], 2);
    assert_eq!(collection["unread_count"], 2);
    assert_eq!(collection["covers"].as_array().unwrap().len(), 2);
    assert_eq!(collection["cover"], collection["covers"][0]);
    assert_eq!(collection["mine"], true);
    assert_eq!(
        collection["made_by"],
        json!({ "id": ben_id, "name": "Ben" })
    );
    assert_eq!(collection["shared_with_all"], false);
    let at = format!("{collections}/{}", collection["id"]);
    let series = format!("{at}/series");
    let sharing = format!("{at}/sharing");
    let held = ben.get(&series).await.json();
    let names: Vec<&str> = held
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Paper Girls", "Saga"]);
    let of_saga = format!("/api/v1/series/{saga}/collections");
    assert_eq!(ben.get(&of_saga).await.json()[0]["id"], collection["id"]);

    // The owner of the server is not shown it either.
    assert_eq!(server.get(collections).await.json(), json!([]));
    assert_eq!(server.get(&at).await.status, StatusCode::NOT_FOUND);
    assert_eq!(server.get(&series).await.status, StatusCode::NOT_FOUND);
    assert_eq!(server.get(&of_saga).await.json(), json!([]));
    let found = server.get("/api/v1/search?q=vaughan").await.json();
    assert_eq!(found["collections"], json!([]));
    for hidden in [
        server.patch(&at, json!({ "name": "Ours" })).await,
        server.post(&series, json!({ "series": [saga] })).await,
        server.put(&sharing, json!({ "everyone": true })).await,
        server.delete(&format!("{series}/{saga}")).await,
        server.delete(&at).await,
    ] {
        assert_refused(&hidden, StatusCode::NOT_FOUND, "not_found", "not shared");
    }

    // A name is taken among one person's collections.
    let hers = server.post(collections, json!({ "name": "Vaughan" })).await;
    assert_eq!(hers.status, StatusCode::CREATED);
    let again = ben.post(collections, json!({ "name": "vaughan" })).await;
    assert_refused(&again, StatusCode::CONFLICT, "conflict", "his own name");
    assert_eq!(again.json()["conflicts_with"], collection["id"]);
    let blank = ben.patch(&at, json!({ "name": "  " })).await;
    assert_refused(
        &blank,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "no name",
    );

    // Its maker gives it other details. Its own name spelled another way
    // is not taken; the name of another of theirs is.
    let respelled = json!({ "name": "VAUGHAN", "summary": "One writer" });
    let changed = ben.patch(&at, respelled).await;
    assert_eq!(changed.status, StatusCode::OK);
    assert_eq!(changed.json()["name"], "VAUGHAN");
    let other = ben
        .post(collections, json!({ "name": "Other" }))
        .await
        .json();
    let onto = json!({ "name": "other", "summary": "One writer" });
    let taken = ben.patch(&at, onto).await;
    assert_refused(&taken, StatusCode::CONFLICT, "conflict", "another of his");
    assert_eq!(taken.json()["conflicts_with"], other["id"]);
    let another = format!("{collections}/{}", other["id"]);
    assert_eq!(ben.delete(&another).await.status, StatusCode::NO_CONTENT);
    assert_eq!(ben.get(&another).await.status, StatusCode::NOT_FOUND);

    // A collection takes the series asked for, or nothing.
    let unknown = ben
        .post(&series, json!({ "series": [manga_series, 9999] }))
        .await;
    assert_refused(
        &unknown,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "unknown",
    );
    let unread = json!({ "name": "Hers", "series": [saga] });
    let refused = cleo.post(collections, unread).await;
    assert_refused(
        &refused,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "a library she does not read",
    );
    let added = ben
        .post(&series, json!({ "series": [manga_series, saga] }))
        .await;
    assert_eq!(added.json(), json!({ "series": 1 }));

    // A series stays in it under another name.
    let renamed = server
        .patch(
            &format!("/api/v1/series/{saga}"),
            json!({ "name": "Saga of Wreath" }),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK);
    let held = ben.get(&series).await.json();
    assert_eq!(held.as_array().unwrap().len(), 3);
    assert!(
        held.as_array()
            .unwrap()
            .iter()
            .any(|one| one["name"] == "Saga of Wreath")
    );
    assert_eq!(ben.get(&of_saga).await.json()[0]["id"], collection["id"]);

    // Shared with the people chosen: they see whose it is, the series
    // they may read, and not who else it was shared with.
    let shared = ben
        .put(&sharing, json!({ "people": [cleo_id, ben_id] }))
        .await;
    assert_eq!(shared.status, StatusCode::OK);
    assert_eq!(
        shared.json()["shared_with"],
        json!([{ "id": cleo_id, "name": "Cleo" }])
    );
    let seen = cleo.get(&at).await.json();
    assert_eq!(seen["mine"], false);
    assert_eq!(seen["made_by"]["name"], "Ben");
    assert_eq!(seen["shared_with"], json!([]));
    assert_eq!(seen["series_count"], 1);
    assert_eq!(cleo.get(&series).await.json().as_array().unwrap().len(), 1);
    assert_eq!(
        cleo.get(collections).await.json()[0]["id"],
        collection["id"]
    );
    assert_eq!(server.get(&at).await.status, StatusCode::NOT_FOUND);
    // It is said with the series she reads, and found by its name. A
    // series she may not read is not there for her to ask about.
    let of_manga = format!("/api/v1/series/{manga_series}/collections");
    assert_eq!(cleo.get(&of_manga).await.json()[0]["id"], collection["id"]);
    assert_eq!(cleo.get(&of_saga).await.status, StatusCode::NOT_FOUND);
    let found = cleo.get("/api/v1/search?q=vaughan").await.json();
    assert_eq!(found["collections"][0]["id"], collection["id"]);
    for refused in [
        cleo.patch(&at, json!({ "name": "Ours" })).await,
        cleo.post(&series, json!({ "series": [manga_series] }))
            .await,
        cleo.put(&sharing, json!({ "everyone": true })).await,
        cleo.delete(&format!("{series}/{manga_series}")).await,
        cleo.delete(&at).await,
    ] {
        assert_refused(
            &refused,
            StatusCode::FORBIDDEN,
            "forbidden",
            "someone else's",
        );
    }
    let nobody = ben.put(&sharing, json!({ "people": [9999] })).await;
    assert_refused(
        &nobody,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "nobody",
    );
    assert_eq!(cleo.get(&at).await.status, StatusCode::OK);

    // One that would show someone no series is not there for them.
    let taken = ben.delete(&format!("{series}/{manga_series}")).await;
    assert_eq!(taken.status, StatusCode::NO_CONTENT);
    let again = ben.delete(&format!("{series}/{manga_series}")).await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
    assert_eq!(cleo.get(&at).await.status, StatusCode::NOT_FOUND);
    assert_eq!(cleo.get(collections).await.json(), json!([]));
    assert_eq!(cleo.get(&of_manga).await.json(), json!([]));
    let found = cleo.get("/api/v1/search?q=vaughan").await.json();
    assert_eq!(found["collections"], json!([]));

    // Shared with everyone, search finds it by its name.
    let shared = ben.put(&sharing, json!({ "everyone": true })).await;
    assert_eq!(shared.json()["shared_with"], json!([]));
    assert_eq!(server.get(&at).await.json()["made_by"]["name"], "Ben");
    let found = server.get("/api/v1/search?q=vaughan").await.json();
    assert_eq!(found["collections"].as_array().unwrap().len(), 2);
    // A sign standing alone among the words is in no name, and asks for
    // nothing of one.
    let found = server.get("/api/v1/search?q=vaughan%20-").await.json();
    assert_eq!(found["collections"].as_array().unwrap().len(), 2);
    let found = server.get("/api/v1/search?q=-").await.json();
    assert_eq!(found["collections"], json!([]));

    // The reading lists a series has a book in are said with it.
    let book = server.first_book(comics).await;
    let list = json!({ "name": "First issues", "books": [book["id"]] });
    assert_eq!(
        ben.post("/api/v1/reading-lists", list).await.status,
        StatusCode::CREATED
    );
    let in_lists = |series: i64| format!("/api/v1/series/{series}/reading-lists");
    assert_eq!(
        ben.get(&in_lists(paper)).await.json()[0]["name"],
        "First issues"
    );
    assert_eq!(ben.get(&in_lists(saga)).await.json(), json!([]));
    assert_eq!(server.get(&in_lists(paper)).await.json(), json!([]));
    assert_eq!(
        cleo.get(&in_lists(paper)).await.status,
        StatusCode::NOT_FOUND
    );

    // A series that is gone leaves the collection, which stays its maker's.
    let removed = server.delete(&format!("/api/v1/libraries/{comics}")).await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    assert_eq!(ben.get(&at).await.json()["series_count"], 0);
    assert_eq!(server.get(&at).await.status, StatusCode::NOT_FOUND);

    // A collection goes with the person who made it.
    let gone = server.delete(&format!("/api/v1/users/{ben_id}")).await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT);
    let left = server.get(collections).await.json();
    assert_eq!(left.as_array().unwrap().len(), 1);
    assert_eq!(left[0]["mine"], true);
}

#[tokio::test]
async fn a_reading_list_is_its_makers_until_it_is_shared() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("Saga/Saga 001.cbz"), 2, 1);
    let comics = server.add_library().await;
    let manga = server.add_second_library("Manga").await;
    let ana = server.get("/api/v1/me").await.json()["id"].clone();
    let ben_id = server.add_user("Ben", &[comics, manga]).await;
    let cleo_id = server.add_user("Cleo", &[manga]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    let cleo = server.signed_in_as("Cleo", PASSWORD).await;
    let saga = server.first_book(comics).await;
    let lists = "/api/v1/reading-lists";

    // Anyone makes a list, and it is theirs alone.
    let new = json!({ "name": "Mine", "books": [saga["id"]] });
    let made = ben.post(lists, new).await;
    assert_eq!(made.status, StatusCode::CREATED);
    let list = made.json();
    assert_eq!(list["mine"], true);
    assert_eq!(list["made_by"], json!({ "id": ben_id, "name": "Ben" }));
    assert_eq!(list["shared_with_all"], false);
    assert_eq!(list["shared_with"], json!([]));
    let at = format!("{lists}/{}", list["id"]);
    let books = format!("{at}/books");
    let sharing = format!("{at}/sharing");
    // The owner of the server is not shown it either.
    assert_eq!(server.get(lists).await.json(), json!([]));
    assert_eq!(server.get(&at).await.status, StatusCode::NOT_FOUND);
    assert_eq!(server.get(&books).await.status, StatusCode::NOT_FOUND);
    let of_saga = format!("/api/v1/books/{}/reading-lists", saga["id"]);
    assert_eq!(server.get(&of_saga).await.json(), json!([]));
    let whole = json!({ "read": true, "lists": [list["id"]] });
    let marked = server.post("/api/v1/read-marks", whole).await;
    assert_eq!(marked.json(), json!({ "books": 0 }));
    let found = server.get("/api/v1/search?q=mine").await.json();
    assert_eq!(found["lists"], json!([]));
    let copied = json!({ "name": "Copy", "lists": [list["id"]] });
    let refused = server.post(lists, copied).await;
    assert_refused(
        &refused,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "a list that is not shown",
    );
    // A cover is the owner's to choose, for a list of the owner's.
    let cover = json!({ "book_id": saga["id"], "page": 1 });
    let refused = ben.put(&format!("{at}/cover"), cover.clone()).await;
    assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", "a member");
    for hidden in [
        server.patch(&at, json!({ "name": "Ours" })).await,
        server.post(&books, json!({ "books": [saga["id"]] })).await,
        server.put(&sharing, json!({ "everyone": true })).await,
        server.delete(&at).await,
    ] {
        assert_refused(&hidden, StatusCode::NOT_FOUND, "not_found", "not shared");
    }

    // A name is taken among one person's lists.
    let hers = server.post(lists, json!({ "name": "Mine" })).await;
    assert_eq!(hers.status, StatusCode::CREATED);
    // The cover chosen for a list is shown to those who see the list.
    let hers = format!("{lists}/{}", hers.json()["id"]);
    let chosen = server.put(&format!("{hers}/cover"), cover.clone()).await;
    assert_eq!(chosen.status, StatusCode::NO_CONTENT);
    let shown = server.get(&hers).await.json()["list"]["cover"].clone();
    let image = format!("/api/v1/covers/{}", shown.as_str().unwrap());
    assert_eq!(server.get(&image).await.status, StatusCode::OK);
    assert_eq!(ben.get(&image).await.status, StatusCode::NOT_FOUND);
    let again = ben.post(lists, json!({ "name": "mine" })).await;
    assert_refused(&again, StatusCode::CONFLICT, "conflict", "his own name");
    assert_eq!(again.json()["conflicts_with"], list["id"]);

    // Everyone is known to everyone by name, and by nothing else.
    assert_eq!(
        cleo.get("/api/v1/users/names").await.json(),
        json!([
            { "id": ana, "name": OWNER },
            { "id": ben_id, "name": "Ben" },
            { "id": cleo_id, "name": "Cleo" },
        ])
    );

    // Shared with the people chosen: they see whose it is, and not who else
    // it was shared with.
    let chosen = json!({ "people": [cleo_id, ana, ben_id] });
    let shared = ben.put(&sharing, chosen).await;
    assert_eq!(shared.status, StatusCode::OK);
    assert_eq!(
        shared.json()["shared_with"],
        json!([{ "id": ana, "name": OWNER }, { "id": cleo_id, "name": "Cleo" }])
    );
    let seen = server.get(&at).await.json()["list"].clone();
    assert_eq!(seen["mine"], false);
    assert_eq!(seen["made_by"]["name"], "Ben");
    assert_eq!(seen["shared_with"], json!([]));
    assert_eq!(seen["book_count"], 1);
    assert_eq!(server.get(&of_saga).await.json()[0]["id"], list["id"]);
    let found = server.get("/api/v1/search?q=mine").await.json();
    assert_eq!(found["lists"].as_array().unwrap().len(), 2);
    // Seeing a list is not changing it, for the owner of the server either.
    for refused in [
        server.patch(&at, json!({ "name": "Ours" })).await,
        server.post(&books, json!({ "books": [saga["id"]] })).await,
        server.put(&sharing, json!({ "everyone": true })).await,
        server.put(&format!("{at}/cover"), cover).await,
        server.delete(&at).await,
    ] {
        assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", "Ben's list");
    }
    // It holds no book Cleo may read, so it is not there for her yet.
    assert_eq!(cleo.get(&at).await.status, StatusCode::NOT_FOUND);
    let hers = cleo.first_book(manga).await;
    let added = ben.post(&books, json!({ "books": [hers["id"]] })).await;
    assert_eq!(added.json(), json!({ "books": 1 }));
    assert_eq!(cleo.get(&at).await.json()["list"]["book_count"], 1);
    assert_eq!(cleo.names_at(&books).await, ["Manga 1"]);

    // Someone who is not on the server is not shared with, and nothing
    // changes.
    let nobody = ben.put(&sharing, json!({ "people": [9999] })).await;
    assert_refused(
        &nobody,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "no such person",
    );
    let still = ben.get(&at).await.json()["list"].clone();
    assert_eq!(still["shared_with"].as_array().unwrap().len(), 2);

    // Sharing says all of it: with everyone and nobody by name, then with
    // nobody at all.
    let everyone = ben.put(&sharing, json!({ "everyone": true })).await;
    assert_eq!(everyone.json()["shared_with_all"], true);
    assert_eq!(everyone.json()["shared_with"], json!([]));
    assert_eq!(cleo.get(&at).await.status, StatusCode::OK);
    assert_eq!(ben.put(&sharing, json!({})).await.status, StatusCode::OK);
    assert_eq!(cleo.get(&at).await.status, StatusCode::NOT_FOUND);
    assert_eq!(server.get(&at).await.status, StatusCode::NOT_FOUND);
    assert_eq!(ben.get(&at).await.json()["list"]["book_count"], 2);

    // A list goes with the one who made it.
    let removed = server.delete(&format!("/api/v1/users/{ben_id}")).await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    let left = server.get(lists).await.json();
    assert_eq!(left.as_array().unwrap().len(), 1);
    assert_eq!(left[0]["made_by"]["name"], OWNER);
}

#[tokio::test]
async fn a_cbl_file_is_matched_fixed_by_hand_and_made_into_a_list() {
    let server = server().await;
    for (seed, file) in [
        "Night Runner/Night Runner 001",
        "Night Runner/Night Runner 002",
        "The Lamplighters/The Lamplighters 001",
        "Harbor Tales/Harbor Tales 003",
    ]
    .iter()
    .enumerate()
    {
        fixtures::write_book(
            &server.comics.join(format!("{file}.cbz")),
            2,
            seed as u8 + 1,
        );
    }
    let comics = server.add_library().await;
    let runner_2 = server.book_named(comics, "Night Runner 002").await;
    let harbor_3 = server.book_named(comics, "Harbor Tales 003").await;

    let file = r#"<?xml version="1.0"?>
        <ReadingList>
          <Name>The Long Night</Name>
          <Books>
            <Book Series="Night Runner" Number="1" Volume="1987" Year="1987" />
            <Book Series="Lamplighters" Number="001" Volume="1990" Year="1990" />
            <Book Series="Harbour Stories" Number="3" Volume="1991" Year="1991" />
            <Book Series="Night Runner" Number="7" Volume="1987" Year="1988" />
          </Books>
        </ReadingList>"#;
    let plan = async |file: &str| {
        let request = Request::post("/api/v1/reading-lists/import")
            .header(header::CONTENT_TYPE, "application/xml");
        let sent = request.body(Body::from(file.to_owned())).unwrap();
        server.send(sent).await
    };
    let found = async || {
        let planned = plan(file).await;
        assert_eq!(planned.status, StatusCode::OK);
        planned.json()
    };

    // Nothing is made; each entry says what it matched and how.
    let planned = found().await;
    assert_eq!(planned["name"], "The Long Night");
    assert!(planned["name_taken_by"].is_null());
    let lines = planned["lines"].as_array().unwrap();
    let told: Vec<_> = lines
        .iter()
        .map(|line| (line["found"].as_str().unwrap(), line["how"].as_str()))
        .collect();
    assert_eq!(
        told,
        [
            ("book", Some("name")),
            ("book", Some("without_article")),
            ("no_series", None),
            ("no_issue", Some("name")),
        ]
    );
    assert_eq!(lines[0]["book"]["name"], "Night Runner 001");
    assert_eq!(lines[0]["entry"]["volume"], "1987");
    assert_eq!(lines[1]["book"]["name"], "The Lamplighters 001");
    assert_eq!(lines[3]["series_name"], "Night Runner");
    assert_eq!(server.get("/api/v1/reading-lists").await.json(), json!([]));

    // The owner says which series a name means, and which book an entry is.
    let rules = "/api/v1/reading-lists/import/rules";
    let series_rule = json!({ "entry": lines[2]["entry"], "series_id": harbor_3["series_id"] });
    let kept = server.post(rules, series_rule).await;
    assert_eq!(kept.status, StatusCode::CREATED);
    let book_rule = json!({ "entry": lines[3]["entry"], "book_id": runner_2["id"] });
    let kept = server.post(rules, book_rule.clone()).await;
    assert_eq!(kept.status, StatusCode::CREATED);
    let rule_id = kept.json()["id"].clone();
    // Said again, it is the same rule.
    assert_eq!(server.post(rules, book_rule).await.json()["id"], rule_id);

    let planned = found().await;
    let lines = planned["lines"].as_array().unwrap();
    for (line, name) in [
        (&lines[2], "Harbor Tales 003"),
        (&lines[3], "Night Runner 002"),
    ] {
        assert_eq!(line["found"], "book", "{name}");
        assert_eq!(line["how"], "rule", "{name}");
        assert_eq!(line["book"]["name"], name);
        assert!(line["rule_id"].is_i64(), "{name}");
    }
    assert_eq!(lines[3]["rule_id"], rule_id);

    // A rule that was wrong is forgotten.
    let rule = format!("{rules}/{rule_id}");
    assert_eq!(server.delete(&rule).await.status, StatusCode::NO_CONTENT);
    let gone = server.delete(&rule).await;
    assert_refused(&gone, StatusCode::NOT_FOUND, "not_found", "no such rule");
    let planned = found().await;
    assert_eq!(planned["lines"][3]["found"], "no_issue");
    assert_eq!(planned["lines"][2]["found"], "book");

    // The list is made from what was settled, like any other.
    let settled: Vec<&Value> = planned["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|line| line["book"].get("id"))
        .collect();
    let new = json!({ "name": planned["name"], "books": settled });
    let made = server.post("/api/v1/reading-lists", new).await;
    assert_eq!(made.status, StatusCode::CREATED);
    let books = format!("/api/v1/reading-lists/{}/books", made.json()["id"]);
    assert_eq!(
        server.names_at(&books).await,
        [
            "Night Runner 001",
            "The Lamplighters 001",
            "Harbor Tales 003"
        ]
    );
    assert_eq!(found().await["name_taken_by"], made.json()["id"]);

    // What cannot be read or kept says why.
    for not_a_list in ["not xml", "<ReadingList><Books /></ReadingList>"] {
        let refused = plan(not_a_list).await;
        assert_refused(
            &refused,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            not_a_list,
        );
    }
    let entry = json!({ "series": "Harbour Stories" });
    for (what, rule) in [
        ("neither", json!({ "entry": entry })),
        (
            "both",
            json!({ "entry": entry, "series_id": 1, "book_id": 1 }),
        ),
        (
            "no such series",
            json!({ "entry": entry, "series_id": 9999 }),
        ),
        (
            "no name",
            json!({ "entry": { "series": " " }, "book_id": runner_2["id"] }),
        ),
    ] {
        let refused = server.post(rules, rule).await;
        assert_refused(&refused, StatusCode::BAD_REQUEST, "invalid_request", what);
    }

    // A member imports too. What the owner said a name means is the
    // owner's: the member's import does not go by it, the name is not taken
    // among their lists, and they keep and forget rules of their own.
    server.add_user("Ben", &[comics]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    let request = Request::post("/api/v1/reading-lists/import");
    let planned = ben.send(request.body(Body::from(file)).unwrap()).await;
    assert_eq!(planned.status, StatusCode::OK);
    let planned = planned.json();
    assert!(planned["name_taken_by"].is_null());
    assert_eq!(planned["lines"][2]["found"], "no_series");
    let his = json!({ "entry": planned["lines"][2]["entry"], "series_id": harbor_3["series_id"] });
    let kept = ben.post(rules, his).await;
    assert_eq!(kept.status, StatusCode::CREATED);
    let his = format!("{rules}/{}", kept.json()["id"]);
    let not_his = server.delete(&his).await;
    assert_refused(&not_his, StatusCode::NOT_FOUND, "not_found", "Ben's rule");
    assert_eq!(found().await["lines"][2]["how"], "rule");
    assert_eq!(ben.delete(&his).await.status, StatusCode::NO_CONTENT);
}

/// The names of the series a path lists a part of.
async fn series_at(server: &Server, path: &str) -> Vec<String> {
    let listed = server.get(path).await;
    assert_eq!(listed.status, StatusCode::OK, "{path}");
    let listed = listed.json();
    let names = listed["series"].as_array().unwrap().iter();
    names
        .map(|series| series["name"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn a_library_is_narrowed_ordered_and_read_a_part_at_a_time() {
    let server = server().await;
    for (seed, path, publisher, year, writer) in [
        (1, "Saga/Saga 001.cbz", "Image", "2012", "Brian K. Vaughan"),
        (
            2,
            "Preacher/Preacher 001.cbz",
            "Vertigo",
            "1995",
            "Garth Ennis",
        ),
        (
            3,
            "The Invisibles/The Invisibles 001.cbz",
            "Vertigo",
            "1994",
            "Grant Morrison",
        ),
        (4, "Y/Y 001.cbz", "Vertigo", "2002", "Brian Vaughan"),
        (
            5,
            "Élodie/Élodie 001.cbz",
            "Small Press, Inc.",
            "1988",
            "Ana Pérez",
        ),
    ] {
        fixtures::write_tagged_book(
            &server.comics.join(path),
            seed,
            &[("Publisher", publisher), ("Year", year), ("Writer", writer)],
        );
    }
    // The same pages under another name and other tags.
    fixtures::write_tagged_book(
        &server.comics.join("Extras/Saga 001 (retagged).cbz"),
        1,
        &[("Title", "Chapter One")],
    );
    let library = server.add_library().await;
    let list = format!("/api/v1/libraries/{library}/series");
    let invisibles = server.book_named(library, "The Invisibles 001").await;
    let read = server
        .put(
            &format!("/api/v1/books/{}/progress", invisibles["id"]),
            json!({ "page": 0 }),
        )
        .await;
    assert_eq!(read.status, StatusCode::OK);

    // Unread Vertigo from the nineties, by an address.
    assert_eq!(
        series_at(&server, &format!("{list}?publisher=Vertigo&year=1990s")).await,
        ["Preacher", "The Invisibles"]
    );
    assert_eq!(
        series_at(
            &server,
            &format!("{list}?read=unread&publisher=Vertigo&year=1990s")
        )
        .await,
        ["Preacher"]
    );
    // A value given twice is either of them, and one ruled out is neither.
    assert_eq!(
        series_at(&server, &format!("{list}?year=1995&year=1980s&sort=year")).await,
        ["Élodie", "Preacher"]
    );
    assert_eq!(
        series_at(
            &server,
            &format!("{list}?not_publisher=Vertigo&not_publisher=Small%20Press%2C%20Inc.")
        )
        .await,
        ["Saga"]
    );
    assert_eq!(
        series_at(&server, &format!("{list}?not_read=unread")).await,
        ["The Invisibles"]
    );
    let bad = server.get(&format!("{list}?year=nineties")).await;
    assert_refused(&bad, StatusCode::BAD_REQUEST, "invalid_request", "year");
    let bad = server.get(&format!("{list}?sort=size")).await;
    assert_refused(&bad, StatusCode::BAD_REQUEST, "invalid_request", "sort");

    // A part at a time, with how many there are in all.
    let part = server
        .get(&format!("{list}?sort=name&reverse=true&offset=1&limit=2"))
        .await
        .json();
    // The copy sits where its book does: it makes no series of its own.
    assert_eq!(part["total"], 5);
    // An accent does not move a name from its letter: Élodie is with the
    // E, so Y is the last by name and the first backwards.
    assert_eq!(part["series"][0]["name"], "The Invisibles");
    assert_eq!(part["series"].as_array().unwrap().len(), 2);
    let shuffled = series_at(&server, &format!("{list}?sort=random&seed=3")).await;
    assert_eq!(
        shuffled,
        series_at(&server, &format!("{list}?sort=random&seed=3")).await
    );

    // The letters there are, under the narrowing in force.
    let letters = server
        .get(&format!("{list}/letters?publisher=Vertigo"))
        .await
        .json();
    assert_eq!(
        letters,
        json!([
            { "letter": "P", "count": 1, "offset": 0 },
            { "letter": "T", "count": 1, "offset": 1 },
            { "letter": "Y", "count": 1, "offset": 2 },
        ])
    );
    let letters = server.get(&format!("{list}/letters")).await.json();
    // A name is filed without its accent, and ordered without it too.
    assert_eq!(
        letters[0],
        json!({ "letter": "E", "count": 1, "offset": 0 })
    );

    let choices = server
        .get(&format!("/api/v1/libraries/{library}/choices"))
        .await
        .json();
    assert_eq!(
        choices["publishers"],
        json!([
            { "name": "Vertigo", "series_count": 3 },
            { "name": "Image", "series_count": 1 },
            { "name": "Small Press, Inc.", "series_count": 1 },
        ])
    );
    assert_eq!(
        choices["decades"],
        json!([
            { "first_year": 1980, "series_count": 1 },
            { "first_year": 1990, "series_count": 2 },
            { "first_year": 2000, "series_count": 1 },
            { "first_year": 2010, "series_count": 1 },
        ])
    );
    assert_eq!(choices["writers"].as_array().unwrap().len(), 5);
    assert_eq!(choices["pencillers"], json!([]));
    let garth = choices["writers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|person| person["name"] == "Garth Ennis")
        .unwrap()["id"]
        .clone();
    assert_eq!(
        series_at(&server, &format!("{list}?writer={garth}")).await,
        ["Preacher"]
    );
    assert!(
        series_at(&server, &format!("{list}?penciller={garth}"))
            .await
            .is_empty()
    );

    // The book held twice, by its pages.
    let held = server
        .get(&format!("/api/v1/libraries/{library}/duplicates"))
        .await
        .json();
    assert_eq!(held.as_array().unwrap().len(), 1);
    let paths: Vec<&str> = held[0]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        ["Extras/Saga 001 (retagged).cbz", "Saga/Saga 001.cbz"]
    );
    assert_eq!(
        held[0]["files"][0]["book"]["identity"],
        held[0]["files"][1]["book"]["identity"]
    );

    // A member is told none of it of a library that is not theirs.
    server.add_user("Ben", &[]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    for path in [
        list.clone(),
        format!("{list}/letters"),
        format!("/api/v1/libraries/{library}/choices"),
    ] {
        let hidden = ben.get(&path).await;
        assert_refused(&hidden, StatusCode::NOT_FOUND, "not_found", &path);
    }
    let theirs = ben
        .get(&format!("/api/v1/libraries/{library}/duplicates"))
        .await;
    assert_refused(&theirs, StatusCode::FORBIDDEN, "forbidden", "duplicates");
}

#[tokio::test]
async fn people_are_found_seen_whole_and_made_one() {
    let server = server().await;
    for (seed, path, writer, penciller) in [
        (1, "Saga/Saga 001.cbz", "Brian K. Vaughan", "Fiona Staples"),
        (2, "Saga/Saga 002.cbz", "Brian K. Vaughan", "Fiona Staples"),
        (3, "Y/Y 001.cbz", "Brian Vaughan (2)", "Pia Guerra"),
    ] {
        fixtures::write_tagged_book(
            &server.comics.join(path),
            seed,
            &[
                ("Writer", writer),
                ("Penciller", penciller),
                ("Year", "2012"),
            ],
        );
    }
    let library = server.add_library().await;
    let made = server
        .post("/api/v1/reading-lists", json!({ "name": "Vaughan's best" }))
        .await;
    assert_eq!(made.status, StatusCode::CREATED);

    // A search finds people by a name they go by, and lists by their name.
    let found = server.get("/api/v1/search?q=vaughan&limit=1").await.json();
    assert_eq!(found["people"].as_array().unwrap().len(), 1);
    assert_eq!(found["more_people"], true);
    assert_eq!(found["people"][0]["name"], "Brian K. Vaughan");
    assert_eq!(found["people"][0]["book_count"], 2);
    assert_eq!(found["lists"][0]["name"], "Vaughan's best");
    let found = server.get("/api/v1/search?q=vaughan").await.json();
    let (brian, other) = (
        found["people"][0]["id"].clone(),
        found["people"][1]["id"].clone(),
    );
    let person = format!("/api/v1/people/{brian}");

    let y = server.book_named(library, "Y 001").await;
    let credits = format!("/api/v1/books/{}/credits", y["id"]);
    assert_eq!(
        server.get(&credits).await.json()["writers"],
        json!([{ "id": other, "name": "Brian Vaughan (2)" }])
    );

    // Only the owner says who is who.
    server.add_user("Ben", &[library]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    let merge = json!({ "people": [other] });
    let refused = ben.post(&format!("{person}/merge"), merge.clone()).await;
    assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", "merge");

    let merged = server.post(&format!("{person}/merge"), merge).await;
    assert_eq!(merged.status, StatusCode::OK);
    let merged = merged.json();
    assert_eq!(merged["name"], "Brian K. Vaughan");
    assert_eq!(merged["book_count"], 3);
    assert_eq!(merged["written_count"], 3);
    assert_eq!(merged["pencilled_count"], 0);
    assert_eq!(merged["series_count"], 2);
    let names: Vec<&str> = merged["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Brian K. Vaughan", "Brian Vaughan (2)"]);
    assert_eq!(ben.get(&person).await.json(), merged);
    let gone = server.get(&format!("/api/v1/people/{other}")).await;
    assert_refused(&gone, StatusCode::NOT_FOUND, "not_found", "merged away");
    assert_eq!(
        server.get(&credits).await.json()["writers"],
        json!([{ "id": brian, "name": "Brian K. Vaughan" }])
    );

    // Their series, and their books a part at a time.
    let series = server.get(&format!("{person}/series")).await.json();
    assert_eq!(series.as_array().unwrap().len(), 2);
    assert_eq!(series[0]["name"], "Saga");
    let books = server
        .get(&format!("{person}/books?offset=1&limit=1"))
        .await
        .json();
    assert_eq!(books["total"], 3);
    assert_eq!(books["books"].as_array().unwrap().len(), 1);
    assert_eq!(books["books"][0]["name"], "Saga 002");

    // One more name, unless someone else goes by it.
    let named = server
        .post(
            &format!("{person}/names"),
            json!({ "name": "B. K. Vaughan" }),
        )
        .await;
    assert_eq!(named.json()["names"].as_array().unwrap().len(), 3);
    let taken = server
        .post(
            &format!("{person}/names"),
            json!({ "name": "fiona staples" }),
        )
        .await;
    assert_refused(&taken, StatusCode::CONFLICT, "conflict", "another's name");
    assert!(taken.json()["conflicts_with"].is_i64());

    // Shown by another name, and the first one a person of its own again.
    let second = merged["names"][1]["id"].clone();
    let first = merged["names"][0]["id"].clone();
    let kept = server
        .delete(&format!("{person}/names/{second}"))
        .await
        .status;
    assert_eq!(kept, StatusCode::OK);
    let again = server
        .post(&format!("{person}/merge"), json!({ "people": [other] }))
        .await;
    assert_refused(&again, StatusCode::NOT_FOUND, "not_found", "a new id");
    let apart = server.get("/api/v1/search?q=vaughan%202").await.json()["people"][0]["id"].clone();
    let merged = server
        .post(&format!("{person}/merge"), json!({ "people": [apart] }))
        .await;
    assert_eq!(merged.status, StatusCode::OK);
    let second = merged.json()["names"]
        .as_array()
        .unwrap()
        .iter()
        .find(|name| name["name"] == "Brian Vaughan (2)")
        .unwrap()["id"]
        .clone();
    let stuck = server.delete(&format!("{person}/names/{first}")).await;
    assert_refused(
        &stuck,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "shown by",
    );
    let shown = server
        .patch(&person, json!({ "shown_by": second }))
        .await
        .json();
    assert_eq!(shown["name"], "Brian Vaughan (2)");
    let apart = server.delete(&format!("{person}/names/{first}")).await;
    assert_eq!(apart.status, StatusCode::OK);
    assert_eq!(apart.json()["name"], "Brian K. Vaughan");
    assert_eq!(server.get(&person).await.json()["book_count"], 1);

    // Someone no book of theirs names is not there for a member.
    server.add_user("Cy", &[]).await;
    let cy = server.signed_in_as("Cy", PASSWORD).await;
    for path in [
        person.clone(),
        format!("{person}/series"),
        format!("{person}/books"),
    ] {
        let hidden = cy.get(&path).await;
        assert_refused(&hidden, StatusCode::NOT_FOUND, "not_found", &path);
    }
    let found = cy.get("/api/v1/search?q=vaughan").await.json();
    assert_eq!(found["people"], json!([]));
    let missing = server.get("/api/v1/people/999").await;
    assert_refused(&missing, StatusCode::NOT_FOUND, "not_found", "nobody");
}

#[tokio::test]
async fn a_view_made_with_a_token_is_seen_by_who_reads_the_library() {
    let server = server().await;
    for (seed, path, publisher, year) in [
        (1, "Sandman/Sandman 001.cbz", "Vertigo", "1991"),
        (2, "Saga/Saga 001.cbz", "Image", "2012"),
    ] {
        fixtures::write_tagged_book(
            &server.comics.join(path),
            seed,
            &[("Publisher", publisher), ("Year", year)],
        );
    }
    let library = server.add_library().await;
    let other = server.add_second_library("Other").await;
    server.add_user("Ben", &[library]).await;
    server.add_user("Eve", &[other]).await;
    assert!(
        server
            .get("/api/v1/views")
            .await
            .json()
            .as_array()
            .unwrap()
            .is_empty()
    );

    // An AI client asks with a token of the owner's, as any app does.
    let (_, secret) = server.make_token("AI client").await;
    let client = server.with_token(&secret);
    let asked = json!({ "sort": "year", "publisher": ["Vertigo"], "year": ["1990s"], "not_read": ["read"] });
    let new = json!({ "library_id": library, "name": " Nineties Vertigo ", "asked": asked, "on_home": true });
    let made = client.post("/api/v1/views", new).await;
    assert_eq!(made.status, StatusCode::CREATED);
    let view = made.json();
    assert_eq!(view["name"], "Nineties Vertigo");
    assert_eq!(
        (&view["on_home"], &view["in_menu"]),
        (&json!(true), &json!(false))
    );
    assert_eq!(view["asked"]["sort"], "year");
    assert_eq!(view["asked"]["publisher"], json!(["Vertigo"]));
    assert_eq!(view["asked"]["year"], json!(["1990s"]));
    assert_eq!(view["asked"]["not_read"], json!(["read"]));
    assert_eq!(view["asked"]["writer"], json!([]));
    let id = view["id"].as_i64().unwrap();

    // Its series are the library's, asked for the same way.
    let series = series_at(
        &server,
        &format!("/api/v1/libraries/{library}/series?sort=year&publisher=Vertigo&year=1990s&not_read=read"),
    )
    .await;
    assert_eq!(series, ["Sandman"]);

    // Whoever may read the library sees it; nobody else does, and only
    // the owner changes it.
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    let eve = server.signed_in_as("Eve", PASSWORD).await;
    assert_eq!(ben.get("/api/v1/views").await.json()[0]["id"], id);
    assert!(
        eve.get("/api/v1/views")
            .await
            .json()
            .as_array()
            .unwrap()
            .is_empty()
    );
    let at = format!("/api/v1/views/{id}");
    let refused = ben.patch(&at, json!({ "in_menu": true })).await;
    assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", "a member");
    let refused = ben
        .post(
            "/api/v1/views",
            json!({ "library_id": library, "name": "Mine" }),
        )
        .await;
    assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", "a member");

    let changed = server
        .patch(
            &at,
            json!({ "in_menu": true, "asked": { "reverse": true } }),
        )
        .await;
    assert_eq!(changed.status, StatusCode::OK);
    let changed = changed.json();
    assert_eq!(changed["in_menu"], true);
    assert_eq!(changed["name"], "Nineties Vertigo");
    assert_eq!(changed["asked"]["reverse"], true);
    assert_eq!(changed["asked"]["publisher"], json!([]));

    // What cannot be kept says why.
    for (new, what) in [
        (json!({ "library_id": library, "name": "  " }), "no name"),
        (
            json!({ "library_id": library, "name": "Odd", "asked": { "year": ["199"] } }),
            "no year",
        ),
    ] {
        let refused = server.post("/api/v1/views", new).await;
        assert_refused(&refused, StatusCode::BAD_REQUEST, "invalid_request", what);
    }
    let refused = server
        .post(
            "/api/v1/views",
            json!({ "library_id": 99, "name": "Nowhere" }),
        )
        .await;
    assert_refused(&refused, StatusCode::NOT_FOUND, "not_found", "no library");

    assert_eq!(server.delete(&at).await.status, StatusCode::NO_CONTENT);
    assert_refused(
        &server.delete(&at).await,
        StatusCode::NOT_FOUND,
        "not_found",
        "gone",
    );
    assert!(
        server
            .get("/api/v1/views")
            .await
            .json()
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn what_someone_chose_follows_them_and_nobody_else() {
    let server = server().await;
    let library = server.add_library().await;
    server.add_user("Ben", &[library]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    let all = "/api/v1/me/settings";
    assert_eq!(server.get(all).await.json(), json!({ "settings": {} }));

    let kept = server
        .put(&format!("{all}/cover-size"), json!({ "value": "large" }))
        .await;
    assert_eq!(kept.status, StatusCode::NO_CONTENT);
    let rows = json!({ "hidden": ["recently-added"] }).to_string();
    ben.put(&format!("{all}/home.rows"), json!({ "value": rows }))
        .await;
    server
        .put(&format!("{all}/cover-size"), json!({ "value": "small" }))
        .await;
    assert_eq!(
        server.get(all).await.json(),
        json!({ "settings": { "cover-size": "small" } })
    );
    assert_eq!(ben.get(all).await.json()["settings"]["home.rows"], rows);

    let refused = server
        .put(&format!("{all}/Cover%20Size"), json!({ "value": "x" }))
        .await;
    assert_refused(
        &refused,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "a key",
    );

    let forgotten = server.delete(&format!("{all}/cover-size")).await;
    assert_eq!(forgotten.status, StatusCode::NO_CONTENT);
    assert_eq!(server.get(all).await.json(), json!({ "settings": {} }));
}

#[tokio::test]
async fn reading_is_looked_back_on_and_the_owner_sees_who_reads_now() {
    let server = server().await;
    fixtures::write_tagged_book(
        &server.comics.join("Saga/Saga 001.cbz"),
        1,
        &[("Writer", "Brian K. Vaughan")],
    );
    let library = server.add_library().await;
    server.add_user("Ben", &[library]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    let book = server.first_book(library).await;

    // The year is asked for on the reader's own clock.
    let year: i64 = 1970
        + std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            / 31_556_952;
    let mine = format!("/api/v1/me/reading?year={year}");
    let nothing = ben.get(&mine).await.json();
    assert_eq!(nothing["first_year"], json!(null));
    assert_eq!(nothing["days"], json!([]));
    assert!(
        server
            .get("/api/v1/reading-now")
            .await
            .json()
            .as_array()
            .unwrap()
            .is_empty()
    );

    let saved = ben
        .put(
            &format!("/api/v1/books/{}/progress", book["id"]),
            json!({ "page": 0 }),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK);
    let read = ben.get(&mine).await.json();
    assert_eq!(read["first_year"], year);
    assert_eq!((&read["books"], &read["pages"]), (&json!(1), &json!(1)));
    assert_eq!(read["days"].as_array().unwrap().len(), 1);
    assert_eq!(read["months"][0]["books"], 1);
    assert_eq!(read["writers"][0]["name"], "Brian K. Vaughan");
    // The owner read nothing: a year is each reader's own.
    assert_eq!(server.get(&mine).await.json()["books"], 0);

    let now = server.get("/api/v1/reading-now").await.json();
    assert_eq!(now[0]["user_name"], "Ben");
    assert_eq!(now[0]["book"]["id"], book["id"]);
    assert_eq!(now[0]["page"], 0);
    let refused = ben.get("/api/v1/reading-now").await;
    assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", "a member");

    let refused = ben.get("/api/v1/me/reading?year=12").await;
    assert_refused(
        &refused,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "a year",
    );
}

#[tokio::test]
async fn the_owner_looks_back_on_scans_and_sets_unreadable_files_aside() {
    let server = server().await;
    let saga = server.comics.join("Saga/Saga 1.cbz");
    fixtures::write_book(&saga, 1, 1);
    std::fs::write(server.comics.join("broken.cbz"), "not a zip").unwrap();
    let id = server.add_library().await;
    server.add_user("Ben", &[id]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;

    let library = format!("/api/v1/libraries/{id}");
    assert_eq!(server.get(&library).await.json()["unreadable_count"], 1);
    let scans = server.get(&format!("{library}/scans")).await.json();
    assert_eq!(scans.as_array().unwrap().len(), 1);
    assert_eq!(
        (&scans[0]["arrived"], &scans[0]["gone"]),
        (&json!(2), &json!(0))
    );

    std::fs::create_dir_all(server.comics.join("Moved")).unwrap();
    std::fs::rename(&saga, server.comics.join("Moved/Saga 1.cbz")).unwrap();
    server.rescan(id).await;
    let scans = server.get(&format!("{library}/scans")).await.json();
    assert_eq!(scans.as_array().unwrap().len(), 2);
    assert_eq!(
        (&scans[0]["moved"], &scans[0]["arrived"]),
        (&json!(1), &json!(0))
    );
    let changes = format!("/api/v1/scans/{}/changes", scans[0]["id"]);
    let moved = server.get(&changes).await.json();
    assert_eq!(moved["total"], 1);
    assert_eq!(moved["scan"]["id"], scans[0]["id"]);
    let change = &moved["changes"][0];
    assert_eq!(change["kind"], "moved");
    assert_eq!(change["path"], "Moved/Saga 1.cbz");
    assert_eq!(change["from_path"], "Saga/Saga 1.cbz");
    assert_eq!(change["book"]["name"], "Saga 1");
    let none = server.get(&format!("{changes}?kind=gone")).await.json();
    assert_eq!((&none["total"], &none["changes"]), (&json!(0), &json!([])));

    // Setting a file aside takes it off the list and out of the count.
    let unreadable = format!("{library}/unreadable");
    let file = server.get(&unreadable).await.json()[0].clone();
    let aside = format!("/api/v1/books/{}/set-aside", file["id"]);
    assert_eq!(
        server.call(Method::PUT, &aside).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(server.get(&unreadable).await.json(), json!([]));
    assert_eq!(server.get(&library).await.json()["unreadable_count"], 0);
    let set_aside = server
        .get(&format!("{unreadable}?set_aside=true"))
        .await
        .json();
    assert_eq!(set_aside[0]["id"], file["id"]);
    assert_eq!(server.get(&library).await.json()["set_aside_count"], 1);
    assert_eq!(server.delete(&aside).await.status, StatusCode::NO_CONTENT);
    assert_eq!(server.get(&library).await.json()["unreadable_count"], 1);

    let readable = server.book_named(id, "Saga 1").await;
    let refused = server
        .call(
            Method::PUT,
            &format!("/api/v1/books/{}/set-aside", readable["id"]),
        )
        .await;
    assert_refused(
        &refused,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "a book",
    );

    // All of it is the owner's. A member is told the files that could not
    // be read, and not how many wait or were set aside.
    for (method, path) in [
        (Method::GET, format!("{library}/scans")),
        (Method::GET, changes),
        (Method::PUT, aside),
        (Method::GET, format!("{unreadable}?set_aside=true")),
    ] {
        let refused = ben.call(method, &path).await;
        assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", &path);
    }
    assert_eq!(ben.get(&unreadable).await.json()[0]["id"], file["id"]);
    let told = ben.get(&library).await.json();
    assert_eq!(
        (&told["unreadable_count"], &told["set_aside_count"]),
        (&json!(null), &json!(null))
    );
}

#[tokio::test]
async fn the_owner_copies_the_database_and_schedules_it() {
    let server = server().await;
    let library = server.add_library().await;
    server.add_user("Ben", &[library]).await;
    let at = "/api/v1/server/backups";
    let before = server.get(at).await.json();
    assert_eq!(
        (&before["scheduled"], &before["kept"]),
        (&json!(true), &json!(7))
    );
    assert_eq!(before["copies"], json!([]));
    assert!(before["folder"].as_str().unwrap().ends_with("backups"));

    let made = server.call(Method::POST, at).await;
    assert_eq!(made.status, StatusCode::CREATED);
    let copy = made.json();
    assert!(copy["name"].as_str().unwrap().starts_with("binge-"));
    assert!(copy["size"].as_u64().unwrap() > 0);
    assert_eq!(server.get(at).await.json()["copies"][0], copy);

    let off = server.patch(at, json!({ "scheduled": false })).await.json();
    assert_eq!(off["scheduled"], false);
    assert_eq!(off["copies"][0], copy);

    let ben = server.signed_in_as("Ben", PASSWORD).await;
    for method in [Method::GET, Method::POST] {
        let refused = ben.call(method, at).await;
        assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", "a member");
    }
}

#[tokio::test]
async fn the_owner_looks_for_a_newer_version_and_asks_for_it() {
    let server = server().await;
    let library = server.add_library().await;
    server.add_user("Ben", &[library]).await;
    let at = "/api/v1/server/update";
    let before = server.get(at).await.json();
    assert_eq!(
        before,
        json!({
            "version": binge_core::VERSION,
            "looks": false,
            "cannot": null,
            "looked_at": null,
            "newer": null,
            "problem": null,
            "not_started": null,
            "stage": "idle",
        })
    );

    // Nobody answers where the test server looks. That is what the look
    // found, not a request that failed.
    let looked = server
        .call(Method::POST, "/api/v1/server/update/look")
        .await;
    assert_eq!(looked.status, StatusCode::OK);
    let looked = looked.json();
    assert!(looked["looked_at"].as_i64().unwrap() > 0);
    assert!(
        looked["problem"]
            .as_str()
            .unwrap()
            .contains("could not reach")
    );
    assert_eq!(looked["newer"], json!(null));

    let on = server.patch(at, json!({ "looks": true })).await.json();
    assert_eq!(on["looks"], true);
    let off = server.patch(at, json!({ "looks": false })).await.json();
    assert_eq!(off["looks"], false);

    // An update names the version a look found, and no look found one.
    let refused = server.post(at, json!({ "version": "9.9.9" })).await;
    assert_refused(&refused, StatusCode::CONFLICT, "conflict", "a version");
    assert_eq!(server.get(at).await.json()["stage"], "idle");

    let ben = server.signed_in_as("Ben", PASSWORD).await;
    for (method, path) in [
        (Method::GET, at),
        (Method::POST, at),
        (Method::POST, "/api/v1/server/update/look"),
    ] {
        let refused = ben.call(method, path).await;
        assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", "a member");
    }
    let refused = ben.patch(at, json!({ "looks": true })).await;
    assert_refused(&refused, StatusCode::FORBIDDEN, "forbidden", "a member");
}

#[tokio::test]
async fn pages_are_kept_and_sent_small() {
    let server = server().await;
    fixtures::write_cbz(
        &server.comics.join("Saga/Saga 001.cbz"),
        &[
            ("01.png", fixtures::page(800, 1200, 1)),
            ("02.png", fixtures::page(1600, 1200, 2)),
        ],
    );
    let library = server.add_library().await;
    server.add_user("Ben", &[library]).await;
    let ben = server.signed_in_as("Ben", PASSWORD).await;
    let book = server.first_book(library).await;
    let (id, identity) = (
        book["id"].as_i64().unwrap(),
        book["identity"].as_str().unwrap(),
    );

    // A page comes small, and is the same picture for good.
    let address = format!("/api/v1/pages/{identity}/1/thumbnail");
    let small = server.get(&address).await;
    assert_eq!(small.status, StatusCode::OK);
    assert_eq!(small.content_type(), "image/jpeg");
    assert_eq!(
        small.headers[header::CACHE_CONTROL],
        "private, max-age=31536000, immutable"
    );
    assert!(small.body.len() < fixtures::page(1600, 1200, 2).len());
    let etag = small.headers[header::ETAG].clone();
    assert_ne!(
        etag,
        server
            .get(&format!("/api/v1/pages/{identity}/1"))
            .await
            .headers[header::ETAG]
    );
    let again = Request::get(&address).header(header::IF_NONE_MATCH, &etag);
    let again = server.send(again.body(Body::empty()).unwrap()).await;
    assert_eq!(again.status, StatusCode::NOT_MODIFIED);
    let missing = server
        .get(&format!("/api/v1/pages/{identity}/2/thumbnail"))
        .await;
    assert_refused(&missing, StatusCode::NOT_FOUND, "not_found", "no such page");

    // Kept pages are each reader's own.
    let kept = format!("/api/v1/pages/{identity}/1/kept");
    assert_eq!(
        ben.call(Method::PUT, &kept).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        ben.call(Method::PUT, &kept).await.status,
        StatusCode::NO_CONTENT
    );
    let of_book = format!("/api/v1/books/{id}/kept-pages");
    assert_eq!(ben.get(&of_book).await.json(), json!([1]));
    assert_eq!(server.get(&of_book).await.json(), json!([]));

    let all = ben.get("/api/v1/me/kept-pages").await.json();
    assert_eq!(all["total"], 1);
    assert_eq!(all["pages"][0]["page"], 1);
    assert_eq!(all["pages"][0]["book"]["id"], id);
    assert!(all["pages"][0]["kept_at"].as_i64().unwrap() > 0);
    assert_eq!(server.get("/api/v1/me/kept-pages").await.json()["total"], 0);

    let none = ben
        .call(Method::PUT, &format!("/api/v1/pages/{identity}/2/kept"))
        .await;
    assert_refused(&none, StatusCode::NOT_FOUND, "not_found", "no such page");

    assert_eq!(ben.delete(&kept).await.status, StatusCode::NO_CONTENT);
    assert_eq!(ben.delete(&kept).await.status, StatusCode::NO_CONTENT);
    assert_eq!(ben.get(&of_book).await.json(), json!([]));

    // Until someone has read enough, nobody can say how long they will take.
    let series = server
        .get(&format!("/api/v1/libraries/{library}/series"))
        .await
        .json()["series"][0]["id"]
        .as_i64()
        .unwrap();
    let overview = server
        .get(&format!("/api/v1/series/{series}/overview"))
        .await
        .json();
    let read = overview["read"].as_object().unwrap();
    assert_eq!(read["seconds_left"], json!(null));
}

#[tokio::test]
async fn want_to_read_is_each_persons_own() {
    let server = server().await;
    fixtures::write_book(&server.comics.join("Saga/Saga 001.cbz"), 2, 1);
    fixtures::write_book(&server.comics.join("Paper Girls/Paper Girls 001.cbz"), 2, 2);
    let comics = server.add_library().await;
    let manga = server.add_second_library("Manga").await;
    server.add_user("Cleo", &[manga]).await;
    let cleo = server.signed_in_as("Cleo", PASSWORD).await;
    let [paper, saga] = server.series_ids(comics).await[..] else {
        panic!("two series");
    };
    let wanted = "/api/v1/want-to-read";
    let of_saga = format!("/api/v1/series/{saga}");
    let off_saga = format!("{wanted}/{saga}");

    // A series named twice is put there once, and one already there stays.
    let none = json!({ "series": [], "total": 0 });
    assert_eq!(server.get(wanted).await.json(), none);
    let put = server
        .post(wanted, json!({ "series": [saga, saga, paper] }))
        .await;
    assert_eq!(put.status, StatusCode::OK);
    assert_eq!(put.json(), json!({ "series": 2 }));
    let again = server.post(wanted, json!({ "series": [saga] })).await;
    assert_eq!(again.json(), json!({ "series": 0 }));
    let theirs = server.get(wanted).await.json();
    assert_eq!(theirs["total"], 2);
    assert_eq!(theirs["series"][0]["name"], "Paper Girls");
    assert_eq!(theirs["series"][1]["name"], "Saga");
    assert_eq!(theirs["series"][1]["wanted"], true);
    // A part of it says how many there are in all.
    let part = server
        .get(&format!("{wanted}?offset=1&limit=1"))
        .await
        .json();
    assert_eq!(part["series"][0]["name"], "Saga");
    assert_eq!(part["total"], 2);
    assert_eq!(server.get(&of_saga).await.json()["wanted"], true);

    // Nobody else is shown it, and a series a person may not read is not
    // put on theirs: nothing of the request is.
    assert_eq!(cleo.get(wanted).await.json(), none);
    let manga_series = server.series_ids(manga).await[0];
    let refused = cleo
        .post(wanted, json!({ "series": [manga_series, saga] }))
        .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(refused.json()["code"], "invalid_request");
    assert_eq!(cleo.get(wanted).await.json(), none);
    assert_eq!(cleo.delete(&off_saga).await.status, StatusCode::NOT_FOUND);

    // A series read to the end is left out, and is still theirs to take
    // off.
    let whole = json!({ "read": true, "series": [saga] });
    let marked = server.post("/api/v1/read-marks", whole).await;
    assert_eq!(marked.status, StatusCode::OK);
    let theirs = server.get(wanted).await.json();
    assert_eq!(theirs["total"], 1);
    assert_eq!(theirs["series"][0]["name"], "Paper Girls");
    assert_eq!(server.get(&of_saga).await.json()["wanted"], true);
    assert_eq!(
        server.delete(&off_saga).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(server.get(&of_saga).await.json()["wanted"], false);
    assert_eq!(server.delete(&off_saga).await.status, StatusCode::NOT_FOUND);
}
