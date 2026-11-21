//! What the scans changed, to look back on: the books that arrived, the
//! ones that moved, the ones that went and the ones that came back.
//!
//! A scan that found a library as it was leaves no record. Paths are
//! relative to the library's folder; nothing here touches the folder.

use std::collections::{HashMap, HashSet};

use sqlx::{QueryBuilder, Sqlite};

use crate::accounts::UserId;
use crate::catalog::{Book, Catalog, CatalogError, PartOf, Span, books, now};
use crate::natural::natural_cmp;
use crate::scan::ROWS_PER_STATEMENT;

/// How many scans of a library are kept to look back on. Older ones are
/// forgotten.
const SCANS_KEPT: i64 = 50;

/// How a scan found a file changed.
///
/// In the order a scan's changes are listed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
pub enum ChangeKind {
    /// A book the library never held.
    Arrived,
    /// A book the library held before, gone since, and here again.
    Back,
    /// A book whose file is at another path.
    Moved,
    /// A book whose file is no longer there.
    Gone,
}

impl ChangeKind {
    /// The kind as a change keeps it.
    fn kept(self) -> &'static str {
        match self {
            Self::Arrived => "arrived",
            Self::Back => "back",
            Self::Moved => "moved",
            Self::Gone => "gone",
        }
    }
}

/// A scan that changed what a library holds, with how many files of each
/// kind it found changed.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct ScanRecord {
    pub id: i64,
    pub library_id: i64,
    /// Seconds since 1970.
    pub started_at: i64,
    pub ended_at: i64,
    pub arrived: i64,
    pub back: i64,
    pub moved: i64,
    pub gone: i64,
}

/// One file a scan found changed.
#[derive(Clone, Debug)]
pub struct ScanChange {
    pub id: i64,
    pub kind: ChangeKind,
    /// Where the file is, or was for one that is gone.
    pub path: String,
    /// Where a moved file was before.
    pub from_path: Option<String>,
    /// The book as the library holds it now. Missing when it holds it no
    /// longer, and for a file that could not be read.
    pub book: Option<Book>,
}

/// A change as it is kept.
#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    kind: ChangeKind,
    path: String,
    from_path: Option<String>,
    content_hash: Option<String>,
}

/// A file a scan saw at a path the library did not know.
pub(crate) struct Arrival<'a> {
    pub(crate) path: &'a str,
    /// The book's identity. Missing for a file that could not be read.
    pub(crate) identity: Option<String>,
    /// Whether the library had a place for the book already.
    pub(crate) held_before: bool,
}

/// A file the library knew that a scan no longer saw.
pub(crate) struct Left<'a> {
    pub(crate) path: &'a str,
    pub(crate) identity: Option<&'a str>,
}

/// A change, as it is written down.
#[derive(Debug, PartialEq, Eq)]
struct Change<'a> {
    kind: ChangeKind,
    path: &'a str,
    from_path: Option<&'a str>,
    identity: Option<&'a str>,
}

/// The name of the file at a path.
pub(crate) fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Tells what a scan changed from what it saw arrive and leave. `here` are
/// the identities of the books whose files stayed where they were.
///
/// A file that arrives as one with the same pages leaves has moved: of
/// several that arrive, the one that kept the file's name first. One that
/// arrives next to a file with the same pages is a copy, which is an
/// arrival. Otherwise it is back when the library kept a place for it, and
/// has arrived when it did not.
fn told<'a>(
    arrivals: &'a [Arrival<'a>],
    left: &'a [Left<'a>],
    mut here: HashSet<&'a str>,
) -> Vec<Change<'a>> {
    let mut left_by_identity: HashMap<&str, Vec<&str>> = HashMap::new();
    for file in left {
        if let Some(identity) = file.identity {
            left_by_identity
                .entry(identity)
                .or_default()
                .push(file.path);
        }
    }
    // Where each arrival came from, when it moved. Those that kept their
    // name are matched before the rest get what is left.
    let mut came_from: Vec<Option<&str>> = vec![None; arrivals.len()];
    for same_name in [true, false] {
        for (file, from) in arrivals.iter().zip(&mut came_from) {
            if from.is_some() {
                continue;
            }
            let Some(gone) = file
                .identity
                .as_deref()
                .and_then(|identity| left_by_identity.get_mut(identity))
            else {
                continue;
            };
            let found = gone
                .iter()
                .position(|path| !same_name || file_name(path) == file_name(file.path));
            *from = found.map(|place| gone.remove(place));
        }
    }
    let moved_from: HashSet<&str> = came_from.iter().flatten().copied().collect();
    // A file that moved is here, so one with the same pages next to it is
    // a copy whichever of the two is told first.
    here.extend(
        arrivals
            .iter()
            .zip(&came_from)
            .filter(|(_, from)| from.is_some())
            .filter_map(|(file, _)| file.identity.as_deref()),
    );
    let mut changes = Vec::with_capacity(arrivals.len() + left.len());
    for (file, from) in arrivals.iter().zip(came_from) {
        let identity = file.identity.as_deref();
        let kind = match (identity, from) {
            (_, Some(_)) => ChangeKind::Moved,
            (Some(identity), None) if !here.contains(identity) && file.held_before => {
                ChangeKind::Back
            }
            _ => ChangeKind::Arrived,
        };
        here.extend(identity);
        changes.push(Change {
            kind,
            path: file.path,
            from_path: from,
            identity,
        });
    }
    changes.extend(
        left.iter()
            .filter(|file| !moved_from.contains(file.path))
            .map(|file| Change {
                kind: ChangeKind::Gone,
                path: file.path,
                from_path: None,
                identity: file.identity,
            }),
    );
    changes
}

impl Catalog {
    /// Writes down what a scan changed, when it changed anything, and
    /// forgets the scans too old to keep.
    pub(crate) async fn record_scan(
        &self,
        library_id: i64,
        started_at: i64,
        arrivals: &[Arrival<'_>],
        left: &[Left<'_>],
        here: HashSet<&str>,
    ) -> Result<(), CatalogError> {
        let mut changes = told(arrivals, left, here);
        if changes.is_empty() {
            return Ok(());
        }
        // Written in the order they are listed in: one kind after another,
        // each by its paths.
        changes.sort_by(|a, b| {
            a.kind
                .cmp(&b.kind)
                .then_with(|| natural_cmp(a.path, b.path))
        });
        let mut tx = self.db.begin_write().await?;
        let scan_id: i64 = sqlx::query_scalar(
            "INSERT INTO scan_record (library_id, started_at, ended_at) VALUES (?, ?, ?) \
             RETURNING id",
        )
        .bind(library_id)
        .bind(started_at)
        .bind(now())
        .fetch_one(&mut *tx)
        .await?;
        for chunk in changes.chunks(ROWS_PER_STATEMENT) {
            let mut insert = QueryBuilder::<Sqlite>::new(
                "INSERT INTO scan_change (scan_id, kind, path, from_path, content_hash) ",
            );
            insert.push_values(chunk, |mut row, change| {
                row.push_bind(scan_id)
                    .push_bind(change.kind)
                    .push_bind(change.path)
                    .push_bind(change.from_path)
                    .push_bind(change.identity);
            });
            insert.build().execute(&mut *tx).await?;
        }
        sqlx::query(
            "DELETE FROM scan_record WHERE library_id = ?1 AND id NOT IN \
             (SELECT id FROM scan_record WHERE library_id = ?1 ORDER BY id DESC LIMIT ?2)",
        )
        .bind(library_id)
        .bind(SCANS_KEPT)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The scans that changed a library, the latest first.
    pub async fn scans_of(
        &self,
        user: UserId,
        library_id: i64,
    ) -> Result<Vec<ScanRecord>, CatalogError> {
        self.library(user, library_id).await?;
        Ok(
            sqlx::query_as(&records("r.library_id = ? ORDER BY r.id DESC"))
                .bind(library_id)
                .fetch_all(self.db.pool())
                .await?,
        )
    }

    /// A part of what one scan changed, of one kind or of all: arrivals
    /// first, then the ones back, moved and gone, each by its paths. Comes
    /// with the scan itself.
    pub async fn scan_changes(
        &self,
        user: UserId,
        scan_id: i64,
        kind: Option<ChangeKind>,
        span: Span,
    ) -> Result<(ScanRecord, PartOf<ScanChange>), CatalogError> {
        let scan: ScanRecord = sqlx::query_as(&records("r.id = ?"))
            .bind(scan_id)
            .fetch_optional(self.db.pool())
            .await?
            .ok_or(CatalogError::NotFound)?;
        self.library(user, scan.library_id).await?;

        // The scan was read with how many of each kind it has.
        let total = match kind {
            None => scan.arrived + scan.back + scan.moved + scan.gone,
            Some(ChangeKind::Arrived) => scan.arrived,
            Some(ChangeKind::Back) => scan.back,
            Some(ChangeKind::Moved) => scan.moved,
            Some(ChangeKind::Gone) => scan.gone,
        };
        // Two statements, so that each is read in order from an index.
        let of_kind = if kind.is_some() {
            "AND kind = ?2"
        } else {
            "AND ?2 IS NULL"
        };
        let rows: Vec<Row> = sqlx::query_as(&format!(
            "SELECT id, kind, path, from_path, content_hash FROM scan_change \
             WHERE scan_id = ?1 {of_kind} ORDER BY id LIMIT ?3 OFFSET ?4"
        ))
        .bind(scan_id)
        .bind(kind)
        .bind(span.limit)
        .bind(span.offset)
        .fetch_all(self.db.pool())
        .await?;

        // The books these are now, where the library still holds them.
        let identities: HashSet<&str> = rows
            .iter()
            .filter_map(|row| row.content_hash.as_deref())
            .collect();
        let mut held: HashMap<String, Vec<Book>> = HashMap::new();
        let mut at_path: HashMap<String, i64> = HashMap::new();
        if !identities.is_empty() {
            // The user and the library are the first two parameters.
            let asked_for: Vec<String> = (0..identities.len())
                .map(|place| format!("?{}", place + 3))
                .collect();
            let these = format!(
                "b.library_id = ?2 AND b.content_hash IN ({})",
                asked_for.join(", ")
            );
            let sql = books(&format!("AND {these} ORDER BY b.id"));
            let mut asked = sqlx::query_as::<_, Book>(&sql)
                .persistent(false)
                .bind(user)
                .bind(scan.library_id);
            // Which file each of them is, to tell two with the same pages
            // apart. It takes the same parameters, and has no use for the first.
            let sql = format!("SELECT b.path, b.id FROM book b WHERE {these}");
            let mut paths = sqlx::query_as::<_, (String, i64)>(&sql)
                .persistent(false)
                .bind(user)
                .bind(scan.library_id);
            for identity in &identities {
                asked = asked.bind(*identity);
                paths = paths.bind(*identity);
            }
            let (books, paths) = tokio::try_join!(
                asked.fetch_all(self.db.pool()),
                paths.fetch_all(self.db.pool())
            )?;
            for book in books {
                if let Some(identity) = book.content_hash.clone() {
                    held.entry(identity).or_default().push(book);
                }
            }
            at_path.extend(paths);
        }
        let items = rows
            .into_iter()
            .map(|row| {
                // Of a book held twice, the file the change is about, or
                // else the one the library had first.
                let book = row.content_hash.and_then(|identity| {
                    let files = held.get(&identity)?;
                    let here = at_path.get(&row.path);
                    let file = files.iter().find(|book| Some(&book.id) == here);
                    file.or(files.first()).cloned()
                });
                ScanChange {
                    id: row.id,
                    kind: row.kind,
                    path: row.path,
                    from_path: row.from_path,
                    book,
                }
            })
            .collect();
        Ok((scan, PartOf { items, total }))
    }
}

/// Selects scans (`r`) with what each changed, counted by kind.
fn records(which: &str) -> String {
    let counted = |kind: ChangeKind| {
        format!(
            "(SELECT COUNT(*) FROM scan_change c WHERE c.scan_id = r.id AND c.kind = '{0}') \
             AS {0}",
            kind.kept()
        )
    };
    format!(
        "SELECT r.id, r.library_id, r.started_at, r.ended_at, {}, {}, {}, {} \
         FROM scan_record r WHERE {which}",
        counted(ChangeKind::Arrived),
        counted(ChangeKind::Back),
        counted(ChangeKind::Moved),
        counted(ChangeKind::Gone),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::scan::tests::{Shelf, shelf};

    /// What the scans of a library changed, the latest first, each change
    /// as (kind, path, where it was, whether the book is held now).
    async fn changes(
        shelf: &Shelf,
        library_id: i64,
    ) -> Vec<Vec<(ChangeKind, String, Option<String>, bool)>> {
        let scans = shelf
            .catalog
            .scans_of(shelf.owner, library_id)
            .await
            .unwrap();
        let mut all = Vec::new();
        for scan in scans {
            let (same, part) = shelf
                .catalog
                .scan_changes(shelf.owner, scan.id, None, Span::asked(0, None))
                .await
                .unwrap();
            assert_eq!(same, scan);
            assert_eq!(
                part.total,
                scan.arrived + scan.back + scan.moved + scan.gone
            );
            all.push(
                part.items
                    .into_iter()
                    .map(|change| {
                        (
                            change.kind,
                            change.path,
                            change.from_path,
                            change.book.is_some(),
                        )
                    })
                    .collect(),
            );
        }
        all
    }

    #[tokio::test]
    async fn a_scan_writes_down_what_arrived_moved_went_and_came_back() {
        let shelf = shelf().await;
        let (first, second) = (
            shelf.comics.join("Saga/Saga 1.cbz"),
            shelf.comics.join("Saga/Saga 2.cbz"),
        );
        fixtures::write_book(&first, 2, 1);
        fixtures::write_book(&second, 2, 2);
        std::fs::write(shelf.comics.join("broken.cbz"), "not a zip").unwrap();
        let id = shelf.add().await;
        let arrived = |path: &str, held| (ChangeKind::Arrived, path.to_owned(), None, held);
        let first_scan = vec![
            arrived("broken.cbz", false),
            arrived("Saga/Saga 1.cbz", true),
            arrived("Saga/Saga 2.cbz", true),
        ];
        assert_eq!(changes(&shelf, id).await, std::slice::from_ref(&first_scan));

        // A scan that finds everything as it was leaves no record.
        shelf.rescan(id).await;
        assert_eq!(changes(&shelf, id).await.len(), 1);

        // One file moves, one goes, and a copy of the first arrives.
        let moved = shelf.comics.join("Elsewhere/Saga 001.cbz");
        std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
        std::fs::rename(&first, &moved).unwrap();
        let kept_aside = shelf.comics.parent().unwrap().join("Saga 2.cbz");
        std::fs::rename(&second, &kept_aside).unwrap();
        std::fs::copy(&moved, shelf.comics.join("Saga/Copy.cbz")).unwrap();
        shelf.rescan(id).await;
        let second_scan = vec![
            arrived("Saga/Copy.cbz", true),
            (
                ChangeKind::Moved,
                "Elsewhere/Saga 001.cbz".to_owned(),
                Some("Saga/Saga 1.cbz".to_owned()),
                true,
            ),
            (ChangeKind::Gone, "Saga/Saga 2.cbz".to_owned(), None, false),
        ];
        // The first scan's arrival that went since is no book any more.
        let mut first_scan = first_scan;
        first_scan[2].3 = false;
        assert_eq!(changes(&shelf, id).await, [second_scan, first_scan]);

        // Of the two files with the same pages, each change is about its own.
        let scans = shelf.catalog.scans_of(shelf.owner, id).await.unwrap();
        let (_, part) = shelf
            .catalog
            .scan_changes(shelf.owner, scans[0].id, None, Span::asked(0, None))
            .await
            .unwrap();
        let files: Vec<&str> = part
            .items
            .iter()
            .filter_map(|change| change.book.as_ref())
            .map(|book| book.name.as_str())
            .collect();
        assert_eq!(files, ["Copy", "Saga 001"]);

        // The one that went comes back, somewhere else.
        std::fs::rename(&kept_aside, shelf.comics.join("Saga 2 again.cbz")).unwrap();
        shelf.rescan(id).await;
        let all = changes(&shelf, id).await;
        assert_eq!(
            all[0],
            [(ChangeKind::Back, "Saga 2 again.cbz".to_owned(), None, true)]
        );
        // What an earlier scan saw go is a book the library holds again.
        let went = (ChangeKind::Gone, "Saga/Saga 2.cbz".to_owned(), None, true);
        assert_eq!(all[1][2], went);

        // One kind at a time, a part at a time.
        let scans = shelf.catalog.scans_of(shelf.owner, id).await.unwrap();
        let (_, part) = shelf
            .catalog
            .scan_changes(
                shelf.owner,
                scans[2].id,
                Some(ChangeKind::Arrived),
                Span::asked(1, Some(1)),
            )
            .await
            .unwrap();
        assert_eq!(part.total, 3);
        assert_eq!(part.items[0].path, "Saga/Saga 1.cbz");

        // Someone who may not read the library finds no scans of it.
        let eve = crate::accounts::testing::member(&shelf.catalog.db, "Eve", &[]).await;
        let refused = shelf.catalog.scans_of(eve, id).await;
        assert!(matches!(refused, Err(CatalogError::NotFound)));
        let refused = shelf
            .catalog
            .scan_changes(eve, scans[0].id, None, Span::asked(0, None))
            .await;
        assert!(matches!(refused, Err(CatalogError::NotFound)));
    }

    #[tokio::test]
    async fn only_the_latest_scans_are_kept() {
        let shelf = shelf().await;
        let id = shelf.add().await;
        for scan in 0..SCANS_KEPT + 2 {
            shelf
                .catalog
                .record_scan(
                    id,
                    scan,
                    &[Arrival {
                        path: "Saga 1.cbz",
                        identity: None,
                        held_before: false,
                    }],
                    &[],
                    HashSet::new(),
                )
                .await
                .unwrap();
        }
        let scans = shelf.catalog.scans_of(shelf.owner, id).await.unwrap();
        assert_eq!(scans.len() as i64, SCANS_KEPT);
        assert_eq!(scans.last().unwrap().started_at, 2);
        let changes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scan_change")
            .fetch_one(shelf.catalog.db.pool())
            .await
            .unwrap();
        assert_eq!(changes, SCANS_KEPT);
    }

    #[test]
    fn of_a_moved_file_and_its_copy_the_one_that_kept_the_name_moved() {
        let arrival = |path| Arrival {
            path,
            identity: Some("one".to_owned()),
            held_before: true,
        };
        let arrivals = [arrival("A copy.cbz"), arrival("Elsewhere/Saga 1.cbz")];
        let left = [Left {
            path: "Saga/Saga 1.cbz",
            identity: Some("one"),
        }];
        let changes: Vec<(ChangeKind, &str, Option<&str>)> = told(&arrivals, &left, HashSet::new())
            .iter()
            .map(|change| (change.kind, change.path, change.from_path))
            .collect();
        assert_eq!(
            changes,
            [
                (ChangeKind::Arrived, "A copy.cbz", None),
                (
                    ChangeKind::Moved,
                    "Elsewhere/Saga 1.cbz",
                    Some("Saga/Saga 1.cbz")
                ),
            ]
        );
    }

    #[test]
    fn two_files_with_the_same_pages_arriving_together_both_arrive() {
        let arrivals = [
            Arrival {
                path: "a.cbz",
                identity: Some("one".to_owned()),
                held_before: false,
            },
            Arrival {
                path: "b.cbz",
                // The first of the two made the place.
                identity: Some("one".to_owned()),
                held_before: true,
            },
        ];
        let kinds: Vec<ChangeKind> = told(&arrivals, &[], HashSet::new())
            .iter()
            .map(|change| change.kind)
            .collect();
        assert_eq!(kinds, [ChangeKind::Arrived, ChangeKind::Arrived]);
    }
}
