//! Want to read: the series a person means to read.
//!
//! It is each person's own and never shared. A series is on it from when
//! they put it there until they take it off, and [`Series::wanted`] says so
//! wherever the series is shown. What they are shown as their want to read
//! leaves out the series they have read to the end: such a series is back
//! by itself when it has a book they have not read.

use crate::accounts::UserId;
use crate::catalog::{
    Catalog, CatalogError, PLACED, PartOf, Series, Span, UNREAD, may_read, now, series,
    series_named,
};
use crate::reading::list;

impl Catalog {
    /// A part of the series `user` wants to read and has not read to the
    /// end, the ones put there last first. Series put there together come
    /// by name.
    pub async fn want_to_read(
        &self,
        user: UserId,
        span: Span,
    ) -> Result<PartOf<Series>, CatalogError> {
        let shown = format!(
            "FROM want_to_read w JOIN series s ON s.id = w.series_id \
             WHERE w.user_id = ?1 AND {} \
             AND EXISTS (SELECT 1 FROM {PLACED} WHERE bp.series_id = s.id AND {UNREAD})",
            may_read("s.library_id")
        );
        let count = format!("SELECT COUNT(*) {shown}");
        // The part is chosen first, so that only its series are filled in.
        let part = format!(
            "SELECT held.* FROM (SELECT s.id, w.wanted_at {shown} \
              ORDER BY w.wanted_at DESC, s.name COLLATE natsort, s.id LIMIT ?2 OFFSET ?3) part \
             JOIN ({}) held ON held.id = part.id \
             ORDER BY part.wanted_at DESC, held.name COLLATE natsort, held.id",
            series("")
        );
        let (total, items) = tokio::try_join!(
            sqlx::query_scalar(&count)
                .bind(user)
                .fetch_one(self.db.pool()),
            sqlx::query_as(&part)
                .bind(user)
                .bind(span.limit)
                .bind(span.offset)
                .fetch_all(self.db.pool()),
        )?;
        Ok(PartOf { items, total })
    }

    /// Puts the series named on `user`'s want to read, and returns how many
    /// that added: a series already on it stays where it was and is not
    /// counted. See [`series_named`] for what is refused.
    pub async fn want(&self, user: UserId, series: &[i64]) -> Result<u64, CatalogError> {
        let mut tx = self.db.begin_write().await?;
        let series = series_named(&mut tx, user, series).await?;
        // The ids are part of the statement, so it is not one to keep.
        let query = format!(
            "INSERT OR IGNORE INTO want_to_read (user_id, series_id, wanted_at) \
             SELECT ?1, s.id, ?2 FROM series s WHERE s.id IN ({})",
            list(&series)
        );
        let put = sqlx::query(&query)
            .persistent(false)
            .bind(user)
            .bind(now())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(put.rows_affected())
    }

    /// Takes a series off `user`'s want to read. One that is not on it, or
    /// that they may not read, is not found: a series whose library they no
    /// longer read is still on it, and looks to them like one that is not
    /// on the server.
    pub async fn want_no_more(&self, user: UserId, series_id: i64) -> Result<(), CatalogError> {
        let query = format!(
            "DELETE FROM want_to_read WHERE user_id = ?1 AND series_id = ?2 \
             AND EXISTS (SELECT 1 FROM series s WHERE s.id = ?2 AND {})",
            may_read("s.library_id")
        );
        let taken_off = sqlx::query(&query)
            .bind(user)
            .bind(series_id)
            .execute(self.db.pool())
            .await?;
        if taken_off.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::tests::shelf;
    use crate::{BooksToMark, accounts, fixtures};

    fn names(series: &[Series]) -> Vec<&str> {
        series.iter().map(|one| one.name.as_str()).collect()
    }

    #[tokio::test]
    async fn want_to_read_is_a_persons_own_and_leaves_out_what_they_read() {
        let shelf = shelf().await;
        for (seed, name) in ["Fables", "Saga", "Y"].into_iter().enumerate() {
            let book = shelf.comics.join(format!("{name}/{name} 1.cbz"));
            fixtures::write_book(&book, 2, seed as u8 + 1);
        }
        let library = shelf.add().await;
        let catalog = &shelf.catalog;
        let owner = shelf.owner;
        let all = catalog.series_in(owner, library).await.unwrap();
        let [fables, saga, y] = [all[0].id, all[1].id, all[2].id];
        let all_of = Span::asked(0, None);
        let wanted = async |user| catalog.want_to_read(user, all_of).await.unwrap().items;

        // A series named twice is put there once, and one already there is
        // not counted. What was put there together comes by name.
        assert_eq!(catalog.want(owner, &[saga, fables, saga]).await.unwrap(), 2);
        assert_eq!(catalog.want(owner, &[saga]).await.unwrap(), 0);
        assert_eq!(names(&wanted(owner).await), ["Fables", "Saga"]);
        // What is put there later comes first.
        sqlx::query("UPDATE want_to_read SET wanted_at = wanted_at - 30")
            .execute(catalog.db.pool())
            .await
            .unwrap();
        assert_eq!(catalog.want(owner, &[y, saga]).await.unwrap(), 1);
        let theirs = wanted(owner).await;
        assert_eq!(names(&theirs), ["Y", "Fables", "Saga"]);
        assert!(theirs.iter().all(|one| one.wanted));

        // A part of it comes with how many there are in all.
        let second = Span::asked(1, Some(1));
        let part = catalog.want_to_read(owner, second).await.unwrap();
        assert_eq!((names(&part.items), part.total), (vec!["Fables"], 3));

        // It is each person's own.
        let ana = accounts::testing::member(&catalog.db, "Ana", &[library]).await;
        assert!(wanted(ana).await.is_empty());
        assert!(!catalog.series(ana, saga).await.unwrap().wanted);
        assert!(matches!(
            catalog.want_no_more(ana, saga).await,
            Err(CatalogError::NotFound)
        ));

        // A series read to the end is left out, and is still wanted: it is
        // back once it has a book that was not read.
        let whole = BooksToMark {
            series: vec![saga],
            ..BooksToMark::default()
        };
        catalog.mark_read(owner, &whole, true).await.unwrap();
        assert_eq!(names(&wanted(owner).await), ["Y", "Fables"]);
        assert!(catalog.series(owner, saga).await.unwrap().wanted);
        fixtures::write_book(&shelf.comics.join("Saga/Saga 2.cbz"), 2, 9);
        shelf.rescan(library).await;
        assert_eq!(names(&wanted(owner).await), ["Y", "Fables", "Saga"]);

        // Taken off, it is on it no more, and cannot be taken off twice.
        catalog.want_no_more(owner, fables).await.unwrap();
        assert_eq!(names(&wanted(owner).await), ["Y", "Saga"]);
        assert!(!catalog.series(owner, fables).await.unwrap().wanted);
        assert!(matches!(
            catalog.want_no_more(owner, fables).await,
            Err(CatalogError::NotFound)
        ));

        // Only a series the person may read is put there, and nothing of a
        // request that names one they may not.
        let eve = accounts::testing::member(&catalog.db, "Eve", &[]).await;
        assert!(matches!(
            catalog.want(eve, &[saga]).await,
            Err(CatalogError::Invalid(_))
        ));
        assert!(matches!(
            catalog.want(owner, &[y, 999]).await,
            Err(CatalogError::Invalid(_))
        ));
    }
}
