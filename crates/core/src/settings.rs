//! What a person chose in an app, kept so that it follows them to another
//! browser or device: the size of the covers, the rows of their home page.
//!
//! The server keeps a value under a key and does not read it. The app that
//! wrote it knows what it means.

use crate::accounts::{AccountError, Accounts, UserId};
use crate::catalog::now;

/// How many settings one person keeps.
const MOST_SETTINGS: i64 = 100;

/// How long a key is at most, in characters.
const MAX_KEY: usize = 64;

/// How long a value is at most, in bytes.
const MAX_VALUE: usize = 8 * 1024;

/// A key is short and plain, so that it can be part of an address.
fn a_key(key: &str) -> Result<&str, AccountError> {
    let plain = |letter: char| {
        letter.is_ascii_lowercase() || letter.is_ascii_digit() || matches!(letter, '.' | '-' | '_')
    };
    if key.is_empty() || key.len() > MAX_KEY || !key.chars().all(plain) {
        return Err(AccountError::Invalid(
            "A setting's key is 1 to 64 lower-case letters, digits, dots, dashes or underscores.",
        ));
    }
    Ok(key)
}

impl Accounts {
    /// Everything `user` chose, by key.
    pub async fn settings(&self, user: UserId) -> Result<Vec<(String, String)>, AccountError> {
        Ok(
            sqlx::query_as("SELECT key, value FROM user_setting WHERE user_id = ? ORDER BY key")
                .bind(user)
                .fetch_all(self.db().pool())
                .await?,
        )
    }

    /// Keeps a value under a key for `user`, in place of the one before.
    pub async fn keep_setting(
        &self,
        user: UserId,
        key: &str,
        value: &str,
    ) -> Result<(), AccountError> {
        let key = a_key(key)?;
        if value.len() > MAX_VALUE {
            return Err(AccountError::Invalid("A setting's value is 8 KB at most."));
        }
        let mut tx = self.db().begin_write().await?;
        let others: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM user_setting WHERE user_id = ? AND key <> ?")
                .bind(user)
                .bind(key)
                .fetch_one(&mut *tx)
                .await?;
        if others >= MOST_SETTINGS {
            return Err(AccountError::Invalid(
                "That is too many settings for one person.",
            ));
        }
        sqlx::query(
            "INSERT INTO user_setting (user_id, key, value, updated_at) VALUES (?, ?, ?, ?) \
             ON CONFLICT (user_id, key) DO UPDATE SET \
             value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(user)
        .bind(key)
        .bind(value)
        .bind(now())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Forgets what `user` kept under a key. Nothing kept there is no error.
    pub async fn forget_setting(&self, user: UserId, key: &str) -> Result<(), AccountError> {
        sqlx::query("DELETE FROM user_setting WHERE user_id = ? AND key = ?")
            .bind(user)
            .bind(a_key(key)?)
            .execute(self.db().pool())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::testing;
    use crate::db::Database;

    #[tokio::test]
    async fn what_a_person_chose_is_kept_for_them_alone() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("binge.db")).await.unwrap();
        let accounts = Accounts::new(db.clone());
        let ana = testing::owner(&db).await;
        let ben = testing::member(&db, "Ben", &[]).await;

        accounts
            .keep_setting(ana, "cover-size", "180")
            .await
            .unwrap();
        accounts.keep_setting(ana, "home.rows", "{}").await.unwrap();
        accounts
            .keep_setting(ana, "cover-size", "200")
            .await
            .unwrap();
        assert_eq!(
            accounts.settings(ana).await.unwrap(),
            [
                ("cover-size".to_owned(), "200".to_owned()),
                ("home.rows".to_owned(), "{}".to_owned())
            ]
        );
        assert!(accounts.settings(ben).await.unwrap().is_empty());

        accounts.forget_setting(ana, "cover-size").await.unwrap();
        accounts.forget_setting(ana, "never-kept").await.unwrap();
        assert_eq!(accounts.settings(ana).await.unwrap().len(), 1);

        for key in ["", "Cover Size", "a/b", &"k".repeat(65)] {
            assert!(matches!(
                accounts.keep_setting(ana, key, "1").await,
                Err(AccountError::Invalid(_))
            ));
        }
        let long = "v".repeat(MAX_VALUE + 1);
        assert!(matches!(
            accounts.keep_setting(ana, "long", &long).await,
            Err(AccountError::Invalid(_))
        ));
    }
}
