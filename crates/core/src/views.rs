//! Views: a library narrowed and ordered, kept under a name.
//!
//! "Unread Vertigo from the nineties" is three clicks; a view keeps them.
//! It holds what the library's own page takes: an order and what narrows
//! the series. Each can be a row on the home page and an entry in the
//! menu. The owner makes them, or an AI client for the owner; whoever may
//! read the library sees them.

use sqlx::SqliteConnection;

use crate::accounts::UserId;
use crate::catalog::{Catalog, CatalogError, may_read, now};
use crate::layout::a_name;
use crate::narrow::{Choice, Narrowing, ReadState, SeriesOrder, SeriesSort, Years};
use crate::reading::list;

/// How many views a server keeps.
const MOST_VIEWS: i64 = 200;

/// How many values one view narrows by.
const MOST_VALUES: usize = 100;

/// A library narrowed and ordered, kept under a name.
#[derive(Clone, Debug)]
pub struct View {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub order: SeriesOrder,
    pub narrowing: Narrowing,
    /// Whether it is a row on the home page.
    pub on_home: bool,
    /// Whether it is an entry in the menu.
    pub in_menu: bool,
}

/// A view to keep.
#[derive(Clone, Debug)]
pub struct NewView {
    pub library_id: i64,
    pub name: String,
    pub order: SeriesOrder,
    pub narrowing: Narrowing,
    pub on_home: bool,
    pub in_menu: bool,
}

/// What to change of a view. What is left out stays.
#[derive(Clone, Debug, Default)]
pub struct ViewChange {
    pub name: Option<String>,
    /// The order and the narrowing go together: they are one address.
    pub asked: Option<(SeriesOrder, Narrowing)>,
    pub on_home: Option<bool>,
    pub in_menu: Option<bool>,
}

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    library_id: i64,
    name: String,
    sort: String,
    reverse: bool,
    seed: u32,
    on_home: bool,
    in_menu: bool,
}

/// The values of a narrowing as they are kept: what is narrowed, the value
/// and whether it is ruled out.
fn kept_values(narrowing: &Narrowing) -> Vec<(&'static str, String, bool)> {
    fn of<T>(
        kind: &'static str,
        choice: &Choice<T>,
        written: impl Fn(&T) -> String,
    ) -> impl Iterator<Item = (&'static str, String, bool)> {
        let asked = choice.asked.iter().map(|value| (value, false));
        let ruled_out = choice.ruled_out.iter().map(|value| (value, true));
        asked
            .chain(ruled_out)
            .map(move |(value, out)| (kind, written(value), out))
    }
    // Named one by one, so that a new thing to narrow by is not left out.
    let Narrowing {
        read,
        publishers,
        years,
        writers,
        pencillers,
    } = narrowing;
    of("read", read, |state| state.kept().to_owned())
        .chain(of("publisher", publishers, String::clone))
        .chain(of("year", years, |years| years.written()))
        .chain(of("writer", writers, i64::to_string))
        .chain(of("penciller", pencillers, i64::to_string))
        .collect()
}

/// Puts one kept value back in a narrowing. A value that reads as nothing
/// any more is left out.
fn take(narrowing: &mut Narrowing, kind: &str, value: String, ruled_out: bool) {
    fn put<T>(choice: &mut Choice<T>, value: Option<T>, ruled_out: bool) {
        if let Some(value) = value {
            match ruled_out {
                true => choice.ruled_out.push(value),
                false => choice.asked.push(value),
            }
        }
    }
    match kind {
        "read" => put(&mut narrowing.read, ReadState::read(&value), ruled_out),
        "publisher" => put(&mut narrowing.publishers, Some(value), ruled_out),
        "year" => put(&mut narrowing.years, Years::parse(&value).ok(), ruled_out),
        "writer" => put(&mut narrowing.writers, value.parse().ok(), ruled_out),
        "penciller" => put(&mut narrowing.pencillers, value.parse().ok(), ruled_out),
        _ => {}
    }
}

/// Writes down what a view narrows by, in place of what it did before.
async fn keep_values(
    tx: &mut SqliteConnection,
    view_id: i64,
    narrowing: &Narrowing,
) -> Result<(), CatalogError> {
    let values = kept_values(narrowing);
    if values.len() > MOST_VALUES {
        return Err(CatalogError::Invalid(
            "A view narrows by at most 100 values.",
        ));
    }
    sqlx::query("DELETE FROM kept_view_value WHERE view_id = ?")
        .bind(view_id)
        .execute(&mut *tx)
        .await?;
    for (kind, value, ruled_out) in values {
        // A value given twice, or both asked for and ruled out, counts once.
        sqlx::query(
            "INSERT OR IGNORE INTO kept_view_value (view_id, kind, value, ruled_out) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(view_id)
        .bind(kind)
        .bind(value)
        .bind(ruled_out)
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}

impl Catalog {
    /// The views of the libraries `user` may read, in name order: all of
    /// them, or only the one with an id.
    async fn views_where(
        &self,
        user: UserId,
        only: Option<i64>,
    ) -> Result<Vec<View>, CatalogError> {
        let query = format!(
            "SELECT v.id, v.library_id, v.name, v.sort, v.reverse, v.seed, v.on_home, v.in_menu \
             FROM kept_view v WHERE {} AND (?2 IS NULL OR v.id = ?2) \
             ORDER BY v.name COLLATE natsort, v.id",
            may_read("v.library_id")
        );
        let rows: Vec<Row> = sqlx::query_as(&query)
            .bind(user)
            .bind(only)
            .fetch_all(self.db.pool())
            .await?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<i64> = rows.iter().map(|row| row.id).collect();
        // The ids are part of the statement, so it is not one to keep.
        let values: Vec<(i64, String, String, bool)> = sqlx::query_as(&format!(
            "SELECT view_id, kind, value, ruled_out FROM kept_view_value \
             WHERE view_id IN ({}) ORDER BY view_id, kind, value COLLATE natsort",
            list(&ids)
        ))
        .persistent(false)
        .fetch_all(self.db.pool())
        .await?;
        let mut views: Vec<View> = rows
            .into_iter()
            .map(|row| View {
                id: row.id,
                library_id: row.library_id,
                name: row.name,
                order: SeriesOrder {
                    sort: SeriesSort::read(&row.sort).unwrap_or_default(),
                    reverse: row.reverse,
                    seed: row.seed,
                },
                narrowing: Narrowing::default(),
                on_home: row.on_home,
                in_menu: row.in_menu,
            })
            .collect();
        for (view_id, kind, value, ruled_out) in values {
            if let Some(view) = views.iter_mut().find(|view| view.id == view_id) {
                take(&mut view.narrowing, &kind, value, ruled_out);
            }
        }
        Ok(views)
    }

    /// The views of the libraries `user` may read, in name order.
    pub async fn views(&self, user: UserId) -> Result<Vec<View>, CatalogError> {
        self.views_where(user, None).await
    }

    pub async fn view(&self, user: UserId, id: i64) -> Result<View, CatalogError> {
        let found = self.views_where(user, Some(id)).await?;
        found.into_iter().next().ok_or(CatalogError::NotFound)
    }

    /// Keeps a view of a library, as `user` who makes it sees it.
    pub async fn add_view(&self, user: UserId, new: NewView) -> Result<View, CatalogError> {
        let name = a_name(&new.name, "A view needs a name.")?;
        self.library(user, new.library_id).await?;
        let mut tx = self.db.begin_write().await?;
        let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kept_view")
            .fetch_one(&mut *tx)
            .await?;
        if kept >= MOST_VIEWS {
            return Err(CatalogError::Invalid(
                "The server keeps 200 views at most. Delete one first.",
            ));
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO kept_view \
             (library_id, name, sort, reverse, seed, on_home, in_menu, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(new.library_id)
        .bind(name)
        .bind(new.order.sort.kept())
        .bind(new.order.reverse)
        .bind(new.order.seed)
        .bind(new.on_home)
        .bind(new.in_menu)
        .bind(now())
        .fetch_one(&mut *tx)
        .await?;
        keep_values(&mut tx, id, &new.narrowing).await?;
        tx.commit().await?;
        self.view(user, id).await
    }

    /// Changes a view, as `user` who changes it sees it.
    pub async fn change_view(
        &self,
        user: UserId,
        id: i64,
        change: ViewChange,
    ) -> Result<View, CatalogError> {
        self.view(user, id).await?;
        let name = match &change.name {
            Some(name) => Some(a_name(name, "A view needs a name.")?),
            None => None,
        };
        let order = change.asked.as_ref().map(|(order, _)| order);
        let mut tx = self.db.begin_write().await?;
        sqlx::query(
            "UPDATE kept_view SET name = COALESCE(?, name), sort = COALESCE(?, sort), \
             reverse = COALESCE(?, reverse), seed = COALESCE(?, seed), \
             on_home = COALESCE(?, on_home), in_menu = COALESCE(?, in_menu) WHERE id = ?",
        )
        .bind(name)
        .bind(order.map(|order| order.sort.kept()))
        .bind(order.map(|order| order.reverse))
        .bind(order.map(|order| order.seed))
        .bind(change.on_home)
        .bind(change.in_menu)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if let Some((_, narrowing)) = &change.asked {
            keep_values(&mut tx, id, narrowing).await?;
        }
        tx.commit().await?;
        self.view(user, id).await
    }

    pub async fn delete_view(&self, id: i64) -> Result<(), CatalogError> {
        let deleted = sqlx::query("DELETE FROM kept_view WHERE id = ?")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        match deleted.rows_affected() {
            0 => Err(CatalogError::NotFound),
            _ => Ok(()),
        }
    }
}

/// Makes the views that narrow by the people in `gone` narrow by `into`,
/// who those people are now one with.
pub(crate) async fn follow_merge(
    tx: &mut SqliteConnection,
    into: i64,
    gone: &[i64],
) -> Result<(), sqlx::Error> {
    // A person's id is kept as text. The ids are part of the statement, so
    // it is not one to keep.
    let gone: Vec<String> = gone.iter().map(|id| format!("'{id}'")).collect();
    let theirs = format!(
        "kind IN ('writer', 'penciller') AND value IN ({})",
        gone.join(", ")
    );
    // A view that already narrows by the one they become keeps that value.
    sqlx::query(&format!(
        "UPDATE OR IGNORE kept_view_value SET value = ? WHERE {theirs}"
    ))
    .persistent(false)
    .bind(into.to_string())
    .execute(&mut *tx)
    .await?;
    sqlx::query(&format!("DELETE FROM kept_view_value WHERE {theirs}"))
        .persistent(false)
        .execute(&mut *tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::scan::tests::{Shelf, shelf};

    async fn library() -> (Shelf, i64) {
        let shelf = shelf().await;
        for (seed, path, publisher, writer) in [
            (1, "Saga/Saga 001.cbz", "Image", "Brian K. Vaughan"),
            (2, "Y/Y 001.cbz", "Vertigo", "Brian Vaughan (2)"),
            (3, "Preacher/Preacher 001.cbz", "Vertigo", "Garth Ennis"),
        ] {
            fixtures::write_tagged_book(
                &shelf.comics.join(path),
                seed,
                &[
                    ("Publisher", publisher),
                    ("Writer", writer),
                    ("Year", "1995"),
                ],
            );
        }
        let library = shelf.add().await;
        (shelf, library)
    }

    fn vertigo(library_id: i64) -> NewView {
        NewView {
            library_id,
            name: "  Unread Vertigo ".to_owned(),
            order: SeriesOrder {
                sort: SeriesSort::Year,
                reverse: true,
                seed: 0,
            },
            narrowing: Narrowing {
                read: Choice {
                    asked: vec![ReadState::Unread],
                    ruled_out: Vec::new(),
                },
                publishers: Choice {
                    asked: vec!["Vertigo".to_owned()],
                    ruled_out: Vec::new(),
                },
                years: Choice {
                    asked: Vec::new(),
                    ruled_out: vec![Years::Decade(1980)],
                },
                ..Narrowing::default()
            },
            on_home: true,
            in_menu: false,
        }
    }

    #[tokio::test]
    async fn a_view_keeps_an_order_and_a_narrowing_under_a_name() {
        let (shelf, library) = library().await;
        let catalog = &shelf.catalog;
        let view = catalog.add_view(shelf.owner, vertigo(library)).await;
        let view = view.unwrap();
        assert_eq!(view.name, "Unread Vertigo");
        assert_eq!(
            (view.order.sort, view.order.reverse),
            (SeriesSort::Year, true)
        );
        assert_eq!(view.narrowing.read.asked, [ReadState::Unread]);
        assert_eq!(view.narrowing.publishers.asked, ["Vertigo"]);
        assert_eq!(view.narrowing.years.ruled_out, [Years::Decade(1980)]);
        assert!(view.on_home && !view.in_menu);

        // It gives what the library's page would with the same asked.
        let all = crate::Span::asked(0, None);
        let part = catalog
            .series_part(shelf.owner, library, &view.narrowing, view.order, all)
            .await
            .unwrap();
        let names: Vec<&str> = part.items.iter().map(|one| one.name.as_str()).collect();
        assert_eq!(names, ["Preacher", "Y"]);

        let change = ViewChange {
            name: Some("Vertigo".to_owned()),
            asked: Some((SeriesOrder::default(), Narrowing::default())),
            in_menu: Some(true),
            ..ViewChange::default()
        };
        let changed = catalog.change_view(shelf.owner, view.id, change).await;
        let changed = changed.unwrap();
        assert_eq!(changed.name, "Vertigo");
        assert_eq!(changed.order.sort, SeriesSort::Name);
        assert!(changed.narrowing.publishers.asked.is_empty());
        assert!(changed.on_home && changed.in_menu);

        assert_eq!(catalog.views(shelf.owner).await.unwrap().len(), 1);
        catalog.delete_view(view.id).await.unwrap();
        assert!(catalog.views(shelf.owner).await.unwrap().is_empty());
        assert!(matches!(
            catalog.delete_view(view.id).await,
            Err(CatalogError::NotFound)
        ));
    }

    #[tokio::test]
    async fn a_view_is_seen_by_who_may_read_its_library_and_needs_a_name() {
        let (shelf, library) = library().await;
        let catalog = &shelf.catalog;
        let view = catalog.add_view(shelf.owner, vertigo(library)).await;
        let view = view.unwrap();
        let ben = crate::accounts::testing::member(&catalog.db, "Ben", &[]).await;
        assert!(catalog.views(ben).await.unwrap().is_empty());
        assert!(matches!(
            catalog.view(ben, view.id).await,
            Err(CatalogError::NotFound)
        ));
        let nameless = NewView {
            name: "  ".to_owned(),
            ..vertigo(library)
        };
        assert!(matches!(
            catalog.add_view(shelf.owner, nameless).await,
            Err(CatalogError::Invalid(_))
        ));
        let nowhere = vertigo(library + 1);
        assert!(matches!(
            catalog.add_view(shelf.owner, nowhere).await,
            Err(CatalogError::NotFound)
        ));
    }

    #[tokio::test]
    async fn a_view_by_someone_follows_them_when_they_are_made_one_with_another() {
        let (shelf, library) = library().await;
        let catalog = &shelf.catalog;
        let everyone = crate::Span::asked(0, None);
        let people = catalog.people(shelf.owner, "vaughan", everyone).await;
        let people = people.unwrap().items;
        let (brian, other) = (people[0].id, people[1].id);
        let by_other = NewView {
            narrowing: Narrowing {
                writers: Choice {
                    asked: vec![other],
                    ruled_out: Vec::new(),
                },
                ..Narrowing::default()
            },
            ..vertigo(library)
        };
        let view = catalog.add_view(shelf.owner, by_other).await.unwrap();
        catalog.merge_people(brian, &[other]).await.unwrap();
        let view = catalog.view(shelf.owner, view.id).await.unwrap();
        assert_eq!(view.narrowing.writers.asked, [brian]);
    }
}
