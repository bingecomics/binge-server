//! Accounts: who may use the server, how they prove it, and which libraries
//! they may read.
//!
//! A password is kept as an Argon2id hash. A session's or a token's secret is
//! random, so a plain BLAKE3 hash of it is kept. Neither the passwords nor
//! the secrets are stored, and nothing here logs them.

use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{self, PasswordHasher, PasswordVerifier};
use sqlx::SqliteConnection;
use tokio::sync::Semaphore;

use crate::catalog::now;
use crate::db::Database;

/// What went wrong. `Display` is for the log; [`AccountError::for_people`]
/// is what to tell the person using the server.
#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("the server already has an owner")]
    AlreadySetUp,
    #[error("the name and password do not match")]
    WrongCredentials,
    #[error("the current password is wrong")]
    WrongPassword,
    #[error("too many passwords are being checked at once")]
    Busy,
    /// Holds the reason, written for the person who typed the value.
    #[error("{0}")]
    Invalid(&'static str),
    #[error("the name is taken")]
    NameTaken,
    #[error("there is no such user")]
    NoSuchUser,
    #[error("there is no such token")]
    NoSuchToken,
    #[error("there is no such library")]
    NoSuchLibrary,
    #[error("the owner's account cannot be changed by this")]
    IsOwner,
    #[error("the password could not be hashed: {0}")]
    Hashing(password_hash::Error),
    #[error("the system gave no random bytes: {0}")]
    Random(getrandom::Error),
    #[error("the database failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("a background task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

impl AccountError {
    /// What to tell the person using the server. The cause goes to the log.
    pub fn for_people(&self) -> &str {
        match self {
            Self::AlreadySetUp => "This server already has an owner. Sign in instead.",
            Self::WrongCredentials => "That name and password do not match.",
            Self::WrongPassword => "The current password is not right.",
            Self::Busy => "Too many sign-in attempts at once. Wait a moment and try again.",
            Self::Invalid(reason) => reason,
            Self::NameTaken => "Someone already has that name.",
            Self::NoSuchUser => "There is no such user.",
            Self::NoSuchToken => "There is no such token.",
            Self::NoSuchLibrary => "One of those libraries is not on the server.",
            Self::IsOwner => {
                "The owner reads every library and cannot be removed. The owner changes \
                 their own password under Account."
            }
            Self::Hashing(_) | Self::Random(_) | Self::Database(_) | Self::Task(_) => {
                crate::FAILED_FOR_PEOPLE
            }
        }
    }
}

/// Names one user. Its own type so it is not taken for a library or a book.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct UserId(pub i64);

/// A person who may use the server.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct User {
    pub id: UserId,
    /// What the person signs in with.
    pub name: String,
    /// The owner reads every library and manages the server.
    pub is_owner: bool,
    /// Seconds since 1970.
    pub created_at: i64,
}

/// A user with the libraries they may read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub user: User,
    /// Ids of the libraries the user may read, in order: every library for
    /// the owner, the ones the owner gave them for anyone else.
    pub libraries: Vec<i64>,
}

/// A token an app uses instead of a password. The secret itself is only
/// known when the token is made.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct ApiToken {
    pub id: i64,
    /// What the person called it.
    pub name: String,
    /// Seconds since 1970.
    pub created_at: i64,
    /// Seconds since 1970, to the minute. Missing when it was never used.
    pub last_used_at: Option<i64>,
}

/// The columns of a [`User`], from the `user` table under the alias `u`.
/// Every query that reads a user takes them from here.
macro_rules! user_columns {
    () => {
        "u.id, u.name, u.is_owner, u.created_at"
    };
}

const USER: &str = concat!("SELECT ", user_columns!(), " FROM user u");
const USER_WITH_HASH: &str = concat!("SELECT ", user_columns!(), ", u.password_hash FROM user u");

/// A user as stored, with what their password is checked against.
#[derive(sqlx::FromRow)]
struct Stored {
    #[sqlx(flatten)]
    user: User,
    password_hash: String,
}

/// The user a session or a token stands for, and when it was last used.
#[derive(sqlx::FromRow)]
struct Holder {
    #[sqlx(flatten)]
    user: User,
    last_used_at: Option<i64>,
}

/// A session nobody used for this long is over. Seconds.
const SESSION_IDLE: i64 = 30 * 24 * 60 * 60;

/// How exact "last used" is. Seconds. Using a session or a token writes to
/// the database at most this often, not on every page image.
const LAST_USED_STEP: i64 = 60;

/// Starts every API token, so one found lying around can be recognized.
const TOKEN_PREFIX: &str = "binge_";

/// Passwords checked at the same time. Hashing is slow on purpose and takes
/// memory; more attempts than this are turned away, not queued.
const CHECKS_AT_ONCE: usize = 4;

/// How long a wrong password holds its place among the checks. With
/// [`CHECKS_AT_ONCE`], this caps guessing at a few passwords a second,
/// whoever asks and however many connections they open.
const FAILED_WAIT: Duration = Duration::from_secs(1);

const MAX_NAME: usize = 64;
const MIN_PASSWORD: usize = 8;
const MAX_PASSWORD: usize = 1024;

/// The accounts of the server.
///
/// Cheap to clone; clones share the database and the limit on password
/// checks.
#[derive(Clone)]
pub struct Accounts {
    db: Database,
    checks: Arc<Semaphore>,
    failed_wait: Duration,
}

impl Accounts {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            checks: Arc::new(Semaphore::new(CHECKS_AT_ONCE)),
            failed_wait: FAILED_WAIT,
        }
    }

    pub(crate) fn db(&self) -> &Database {
        &self.db
    }

    /// For tests: a wrong password is answered at once.
    #[cfg(any(test, feature = "fixtures"))]
    pub fn without_waits(mut self) -> Self {
        self.failed_wait = Duration::ZERO;
        self
    }

    /// Whether the server still waits for its owner to be made.
    pub async fn needs_setup(&self) -> Result<bool, AccountError> {
        Ok(
            sqlx::query_scalar("SELECT NOT EXISTS (SELECT 1 FROM user WHERE is_owner)")
                .fetch_one(self.db.pool())
                .await?,
        )
    }

    /// First-run setup: makes the owner, gives them the progress that was
    /// saved before the server had accounts, and signs them in. Returns the
    /// owner and the secret of their session.
    pub async fn create_owner(
        &self,
        name: &str,
        password: &str,
    ) -> Result<(User, String), AccountError> {
        let name = valid_name(name)?;
        valid_password(password)?;
        // Asked before the hashing, so that a setup that cannot succeed
        // costs nothing.
        if !self.needs_setup().await? {
            return Err(AccountError::AlreadySetUp);
        }
        let password_hash = hashed(password).await?;

        let mut tx = self.db.begin_write().await?;
        let id = insert_user(&mut tx, name, &password_hash, true)
            .await
            // Another setup got there first.
            .map_err(|error| if_unique(error, AccountError::AlreadySetUp))?;
        sqlx::query(
            "INSERT INTO progress (user_id, content_hash, page, completed, updated_at) \
             SELECT ?, content_hash, page, completed, updated_at FROM progress_before_accounts",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM progress_before_accounts")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        let owner = self.user(id).await?;
        tracing::info!(name = %owner.name, "the owner's account was created");
        let secret = self.open_session(id).await?;
        Ok((owner, secret))
    }

    /// Checks a name and password, and starts a session. Returns the user
    /// and the session's secret, which goes to the browser and nowhere else.
    pub async fn sign_in(
        &self,
        name: &str,
        password: &str,
    ) -> Result<(User, String), AccountError> {
        let query = format!("{USER_WITH_HASH} WHERE u.name_key = ?");
        let stored: Option<Stored> = sqlx::query_as(&query)
            .bind(name_key(name.trim()))
            .fetch_optional(self.db.pool())
            .await?;
        let Some(user) = self.check_password(stored, password).await? else {
            return Err(AccountError::WrongCredentials);
        };
        let secret = self.open_session(user.id).await?;
        Ok((user, secret))
    }

    /// The user when the password is theirs. A name nobody has takes as long
    /// to refuse as a wrong password, so the answer does not say which names
    /// exist.
    async fn check_password(
        &self,
        stored: Option<Stored>,
        password: &str,
    ) -> Result<Option<User>, AccountError> {
        let turn = Arc::clone(&self.checks)
            .try_acquire_owned()
            .map_err(|_| AccountError::Busy)?;
        let (user, hash) = match stored {
            Some(stored) => (Some(stored.user), Some(stored.password_hash)),
            None => (None, None),
        };
        // Longer than any password that could have been set: not worth the
        // hashing, but still worth the wait.
        let password = (password.chars().count() <= MAX_PASSWORD).then(|| password.to_owned());
        let failed_wait = self.failed_wait;
        // A task of its own, which holds the check's place to the end: the
        // hashing, and the wait after a wrong password. Whoever asked cannot
        // give the place back early by no longer waiting for the answer.
        let right = tokio::spawn(async move {
            let _turn = turn;
            let right = tokio::task::spawn_blocking(move || match (password, hash) {
                (Some(password), Some(hash)) => matches(&password, &hash),
                (Some(password), None) => matches(&password, nobodys_hash()?).map(|_| false),
                (None, _) => Ok(false),
            })
            .await??;
            if !right {
                tokio::time::sleep(failed_wait).await;
            }
            Ok::<bool, AccountError>(right)
        })
        .await??;
        Ok(user.filter(|_| right))
    }

    async fn open_session(&self, user: UserId) -> Result<String, AccountError> {
        let secret = new_secret()?;
        let now = now();
        // The sessions nobody came back to.
        sqlx::query("DELETE FROM session WHERE last_used_at < ?")
            .bind(now - SESSION_IDLE)
            .execute(self.db.pool())
            .await?;
        sqlx::query(
            "INSERT INTO session (token_hash, user_id, created_at, last_used_at) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(hash_of(&secret))
        .bind(user)
        .bind(now)
        .bind(now)
        .execute(self.db.pool())
        .await?;
        Ok(secret)
    }

    /// The user signed in with this session, if the session is still good.
    pub async fn session_user(&self, secret: &str) -> Result<Option<User>, AccountError> {
        let now = now();
        let hash = hash_of(secret);
        let holder: Option<Holder> = sqlx::query_as(concat!(
            "SELECT ",
            user_columns!(),
            ", s.last_used_at FROM session s JOIN user u ON u.id = s.user_id \
             WHERE s.token_hash = ? AND s.last_used_at >= ?"
        ))
        .bind(&hash)
        .bind(now - SESSION_IDLE)
        .fetch_optional(self.db.pool())
        .await?;
        self.used("session", holder, &hash, now).await
    }

    /// The user this API token was made by, if it was not revoked.
    pub async fn token_user(&self, secret: &str) -> Result<Option<User>, AccountError> {
        let hash = hash_of(secret);
        let holder: Option<Holder> = sqlx::query_as(concat!(
            "SELECT ",
            user_columns!(),
            ", t.last_used_at FROM api_token t JOIN user u ON u.id = t.user_id \
             WHERE t.token_hash = ?"
        ))
        .bind(&hash)
        .fetch_optional(self.db.pool())
        .await?;
        self.used("api_token", holder, &hash, now()).await
    }

    /// Notes that a session or a token was used just now, and gives its user.
    async fn used(
        &self,
        table: &'static str,
        holder: Option<Holder>,
        hash: &str,
        now: i64,
    ) -> Result<Option<User>, AccountError> {
        let Some(holder) = holder else {
            return Ok(None);
        };
        if holder
            .last_used_at
            .is_none_or(|last| now - last >= LAST_USED_STEP)
        {
            let query = format!("UPDATE {table} SET last_used_at = ? WHERE token_hash = ?");
            sqlx::query(&query)
                .bind(now)
                .bind(hash)
                .execute(self.db.pool())
                .await?;
        }
        Ok(Some(holder.user))
    }

    /// Ends a session. Nothing happens when it is already over.
    pub async fn sign_out(&self, secret: &str) -> Result<(), AccountError> {
        sqlx::query("DELETE FROM session WHERE token_hash = ?")
            .bind(hash_of(secret))
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// Changes a user's own password, which takes the current one. Their
    /// other sessions end; `keep_session` is the secret of the one they are
    /// making the change from. Their API tokens stay.
    pub async fn change_password(
        &self,
        user: UserId,
        current: &str,
        new: &str,
        keep_session: Option<&str>,
    ) -> Result<(), AccountError> {
        valid_password(new)?;
        let query = format!("{USER_WITH_HASH} WHERE u.id = ?");
        let stored: Option<Stored> = sqlx::query_as(&query)
            .bind(user)
            .fetch_optional(self.db.pool())
            .await?;
        if self.check_password(stored, current).await?.is_none() {
            return Err(AccountError::WrongPassword);
        }
        let password_hash = hashed(new).await?;
        let mut tx = self.db.begin_write().await?;
        set_password(&mut tx, user, &password_hash, keep_session).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Makes an API token for a user. Returns it with its secret, which is
    /// not kept and cannot be shown again.
    pub async fn create_token(
        &self,
        user: UserId,
        name: &str,
    ) -> Result<(ApiToken, String), AccountError> {
        let name = valid_label(name, "Give the token a name, such as the device it is for.")?;
        let secret = format!("{TOKEN_PREFIX}{}", new_secret()?);
        let token: ApiToken = sqlx::query_as(
            "INSERT INTO api_token (user_id, name, token_hash, created_at) VALUES (?, ?, ?, ?) \
             RETURNING id, name, created_at, last_used_at",
        )
        .bind(user)
        .bind(name)
        .bind(hash_of(&secret))
        .bind(now())
        .fetch_one(self.db.pool())
        .await?;
        Ok((token, secret))
    }

    /// A user's API tokens, the newest first.
    pub async fn tokens(&self, user: UserId) -> Result<Vec<ApiToken>, AccountError> {
        Ok(sqlx::query_as(
            "SELECT id, name, created_at, last_used_at FROM api_token \
             WHERE user_id = ? ORDER BY id DESC",
        )
        .bind(user)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// Revokes one of a user's API tokens. It stops working at once.
    pub async fn revoke_token(&self, user: UserId, id: i64) -> Result<(), AccountError> {
        let revoked = sqlx::query("DELETE FROM api_token WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user)
            .execute(self.db.pool())
            .await?;
        if revoked.rows_affected() == 0 {
            return Err(AccountError::NoSuchToken);
        }
        Ok(())
    }

    async fn user(&self, id: UserId) -> Result<User, AccountError> {
        let query = format!("{USER} WHERE u.id = ?");
        sqlx::query_as(&query)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?
            .ok_or(AccountError::NoSuchUser)
    }

    /// Makes sure there is a user with this id who is not the owner. What
    /// the owner decides about the others is not for the owner's own
    /// account.
    async fn not_the_owner(&self, id: UserId) -> Result<(), AccountError> {
        if self.user(id).await?.is_owner {
            return Err(AccountError::IsOwner);
        }
        Ok(())
    }

    /// A user with the libraries they may read.
    pub async fn account(&self, id: UserId) -> Result<Account, AccountError> {
        let user = self.user(id).await?;
        let libraries = sqlx::query_scalar(
            "SELECT l.id FROM library l WHERE ?1 OR EXISTS \
             (SELECT 1 FROM library_access a WHERE a.library_id = l.id AND a.user_id = ?2) \
             ORDER BY l.id",
        )
        .bind(user.is_owner)
        .bind(id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(Account { user, libraries })
    }

    /// Every user with the libraries they may read: the owner, then the
    /// others by name.
    pub async fn accounts(&self) -> Result<Vec<Account>, AccountError> {
        let query = format!("{USER} ORDER BY u.is_owner DESC, u.name COLLATE natsort");
        let users: Vec<User> = sqlx::query_as(&query).fetch_all(self.db.pool()).await?;
        let every: Vec<i64> = sqlx::query_scalar("SELECT id FROM library ORDER BY id")
            .fetch_all(self.db.pool())
            .await?;
        let access: Vec<(UserId, i64)> =
            sqlx::query_as("SELECT user_id, library_id FROM library_access ORDER BY library_id")
                .fetch_all(self.db.pool())
                .await?;
        Ok(users
            .into_iter()
            .map(|user| Account {
                libraries: if user.is_owner {
                    every.clone()
                } else {
                    access
                        .iter()
                        .filter(|(user_id, _)| *user_id == user.id)
                        .map(|(_, library)| *library)
                        .collect()
                },
                user,
            })
            .collect())
    }

    /// Adds a user who may read the given libraries.
    pub async fn create_user(
        &self,
        name: &str,
        password: &str,
        libraries: &[i64],
    ) -> Result<Account, AccountError> {
        let name = valid_name(name)?;
        valid_password(password)?;
        let password_hash = hashed(password).await?;

        let mut tx = self.db.begin_write().await?;
        let id = insert_user(&mut tx, name, &password_hash, false)
            .await
            .map_err(|error| if_unique(error, AccountError::NameTaken))?;
        set_libraries(&mut tx, id, libraries).await?;
        tx.commit().await?;
        self.account(id).await
    }

    /// Changes what the owner decides about another user: a new password,
    /// which ends that user's sessions, and the libraries they may read.
    /// What is `None` stays as it is.
    pub async fn change_user(
        &self,
        id: UserId,
        password: Option<&str>,
        libraries: Option<&[i64]>,
    ) -> Result<Account, AccountError> {
        self.not_the_owner(id).await?;
        let password_hash = match password {
            Some(password) => {
                valid_password(password)?;
                Some(hashed(password).await?)
            }
            None => None,
        };

        let mut tx = self.db.begin_write().await?;
        if let Some(password_hash) = password_hash {
            set_password(&mut tx, id, &password_hash, None).await?;
        }
        if let Some(libraries) = libraries {
            set_libraries(&mut tx, id, libraries).await?;
        }
        tx.commit().await?;
        self.account(id).await
    }

    /// Removes a user with their sessions, tokens and reading progress. The
    /// owner cannot be removed.
    pub async fn remove_user(&self, id: UserId) -> Result<(), AccountError> {
        self.not_the_owner(id).await?;
        sqlx::query("DELETE FROM user WHERE id = ?")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }
}

/// Adds a user and returns their id. The database refuses a second user of
/// the same name, and a second owner.
async fn insert_user(
    tx: &mut SqliteConnection,
    name: &str,
    password_hash: &str,
    is_owner: bool,
) -> Result<UserId, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO user (name, name_key, password_hash, is_owner, created_at) \
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(name)
    .bind(name_key(name))
    .bind(password_hash)
    .bind(is_owner)
    .bind(now())
    .fetch_one(tx)
    .await
}

/// What names are compared by, so that "Ana", "ana" and "ANA" are one name,
/// and "Élodie" and "élodie" too.
fn name_key(name: &str) -> String {
    name.to_lowercase()
}

/// Stores a new password hash and ends the user's sessions, except the one
/// whose secret is `keep_session`.
async fn set_password(
    tx: &mut SqliteConnection,
    user: UserId,
    password_hash: &str,
    keep_session: Option<&str>,
) -> Result<(), AccountError> {
    sqlx::query("UPDATE user SET password_hash = ? WHERE id = ?")
        .bind(password_hash)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    // Without one to keep, `IS NOT NULL` holds for all of them.
    sqlx::query("DELETE FROM session WHERE user_id = ? AND token_hash IS NOT ?")
        .bind(user)
        .bind(keep_session.map(hash_of))
        .execute(&mut *tx)
        .await?;
    Ok(())
}

/// Makes `libraries` the libraries the user may read.
async fn set_libraries(
    tx: &mut SqliteConnection,
    user: UserId,
    libraries: &[i64],
) -> Result<(), AccountError> {
    sqlx::query("DELETE FROM library_access WHERE user_id = ?")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    for library in libraries {
        sqlx::query("INSERT OR IGNORE INTO library_access (user_id, library_id) VALUES (?, ?)")
            .bind(user)
            .bind(library)
            .execute(&mut *tx)
            .await
            .map_err(|error| match &error {
                sqlx::Error::Database(refused) if refused.is_foreign_key_violation() => {
                    AccountError::NoSuchLibrary
                }
                _ => error.into(),
            })?;
    }
    Ok(())
}

/// `taken` when the database refused a row because it already has one like
/// it.
fn if_unique(error: sqlx::Error, taken: AccountError) -> AccountError {
    match &error {
        sqlx::Error::Database(refused) if refused.is_unique_violation() => taken,
        _ => error.into(),
    }
}

/// A name someone can sign in with, without the spaces around it.
fn valid_name(name: &str) -> Result<&str, AccountError> {
    valid_label(name, "Give a name to sign in with.")
}

/// A short text a person typed to name something, without the spaces around
/// it. `when_empty` is what to say when there is nothing left.
fn valid_label<'a>(text: &'a str, when_empty: &'static str) -> Result<&'a str, AccountError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AccountError::Invalid(when_empty));
    }
    if text.chars().count() > MAX_NAME {
        return Err(AccountError::Invalid("A name is at most 64 characters."));
    }
    if text.chars().any(char::is_control) {
        return Err(AccountError::Invalid(
            "A name cannot hold line breaks or other control characters.",
        ));
    }
    Ok(text)
}

fn valid_password(password: &str) -> Result<(), AccountError> {
    match password.chars().count() {
        length if length < MIN_PASSWORD => Err(AccountError::Invalid(
            "A password needs at least 8 characters.",
        )),
        length if length > MAX_PASSWORD => Err(AccountError::Invalid(
            "A password is at most 1024 characters.",
        )),
        _ => Ok(()),
    }
}

/// The Argon2id hash of a password, as a PHC string with its own salt.
/// Computed off the async threads, because it is slow on purpose.
async fn hashed(password: &str) -> Result<String, AccountError> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || hash(&password)).await?
}

fn hash(password: &str) -> Result<String, AccountError> {
    let hash: PasswordHash = Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(AccountError::Hashing)?;
    Ok(hash.to_string())
}

/// Whether `password` is the one `hash` was made from.
fn matches(password: &str, hash: &str) -> Result<bool, AccountError> {
    let hash = PasswordHash::new(hash).map_err(|error| AccountError::Hashing(error.into()))?;
    match Argon2::default().verify_password(password.as_bytes(), &hash) {
        Ok(()) => Ok(true),
        Err(password_hash::Error::PasswordInvalid) => Ok(false),
        Err(error) => Err(AccountError::Hashing(error)),
    }
}

/// A hash no password matches, to spend the same time on a name nobody has
/// as on a wrong password.
fn nobodys_hash() -> Result<&'static str, AccountError> {
    static HASH: OnceLock<String> = OnceLock::new();
    if let Some(hash) = HASH.get() {
        return Ok(hash);
    }
    let made = hash(&new_secret()?)?;
    Ok(HASH.get_or_init(|| made))
}

/// 32 random bytes from the operating system, as 64 hexadecimal digits.
fn new_secret() -> Result<String, AccountError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(AccountError::Random)?;
    let mut secret = String::with_capacity(2 * bytes.len());
    for byte in bytes {
        // Writing to a String cannot fail.
        let _ = write!(secret, "{byte:02x}");
    }
    Ok(secret)
}

/// What the database keeps of a session's or token's secret. The secret is
/// 256 random bits, so a fast hash is enough: there is nothing to guess.
fn hash_of(secret: &str) -> String {
    blake3::hash(secret.as_bytes()).to_hex().to_string()
}

/// Users made straight in the database, for tests that are not about
/// passwords.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    pub(crate) async fn owner(db: &Database) -> UserId {
        user(db, "owner", true).await
    }

    /// A user who is not the owner and may read the given libraries.
    pub(crate) async fn member(db: &Database, name: &str, libraries: &[i64]) -> UserId {
        let id = user(db, name, false).await;
        let mut connection = db.pool().acquire().await.unwrap();
        set_libraries(&mut connection, id, libraries).await.unwrap();
        id
    }

    async fn user(db: &Database, name: &str, is_owner: bool) -> UserId {
        let mut connection = db.pool().acquire().await.unwrap();
        insert_user(&mut connection, name, "not a hash", is_owner)
            .await
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORD: &str = "correct horse";

    struct House {
        _dir: tempfile::TempDir,
        db: Database,
        accounts: Accounts,
    }

    async fn house() -> House {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("binge.db")).await.unwrap();
        House {
            accounts: Accounts::new(db.clone()).without_waits(),
            db,
            _dir: dir,
        }
    }

    impl House {
        async fn owner(&self) -> (User, String) {
            self.accounts.create_owner("Ana", PASSWORD).await.unwrap()
        }

        async fn library(&self, name: &str) -> i64 {
            sqlx::query_scalar(
                "INSERT INTO library (name, root_path, created_at) VALUES (?, ?, 0) RETURNING id",
            )
            .bind(name)
            .bind(format!("/comics/{name}"))
            .fetch_one(self.db.pool())
            .await
            .unwrap()
        }

        async fn count(&self, table: &str) -> i64 {
            sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(self.db.pool())
                .await
                .unwrap()
        }
    }

    #[tokio::test]
    async fn setup_makes_one_owner_and_signs_them_in() {
        let house = house().await;
        assert!(house.accounts.needs_setup().await.unwrap());

        let (owner, session) = house.owner().await;
        assert!(owner.is_owner);
        assert_eq!(owner.name, "Ana");
        assert!(!house.accounts.needs_setup().await.unwrap());
        assert_eq!(
            house.accounts.session_user(&session).await.unwrap(),
            Some(owner)
        );

        let again = house.accounts.create_owner("Eve", PASSWORD).await;
        assert!(matches!(again, Err(AccountError::AlreadySetUp)));
        assert_eq!(house.count("user").await, 1);
    }

    #[tokio::test]
    async fn names_and_passwords_have_rules() {
        let house = house().await;
        for (name, password) in [
            ("", PASSWORD),
            ("   ", PASSWORD),
            ("two\nlines", PASSWORD),
            (&"n".repeat(65), PASSWORD),
            ("Ana", "short"),
            ("Ana", &"p".repeat(1025)),
        ] {
            let refused = house.accounts.create_owner(name, password).await;
            assert!(matches!(refused, Err(AccountError::Invalid(_))), "{name:?}");
        }
        assert!(house.accounts.needs_setup().await.unwrap());

        // The sentences name the limits; they have to be the same numbers.
        let told = |refused: Result<(), AccountError>| refused.unwrap_err().for_people().to_owned();
        assert!(told(valid_password("short")).contains(&format!(" {MIN_PASSWORD} ")));
        let long = "p".repeat(MAX_PASSWORD + 1);
        assert!(told(valid_password(&long)).contains(&format!(" {MAX_PASSWORD} ")));
        let long = "n".repeat(MAX_NAME + 1);
        assert!(told(valid_name(&long).map(|_| ())).contains(&format!(" {MAX_NAME} ")));

        // Spaces around a name are not part of it; eight characters are
        // enough, whatever they are.
        let (owner, _) = house
            .accounts
            .create_owner("  Ana  ", "pässwörd")
            .await
            .unwrap();
        assert_eq!(owner.name, "Ana");
    }

    #[tokio::test]
    async fn only_the_hash_of_a_password_is_stored() {
        let house = house().await;
        house.owner().await;
        let stored: String = sqlx::query_scalar("SELECT password_hash FROM user")
            .fetch_one(house.db.pool())
            .await
            .unwrap();
        assert!(stored.starts_with("$argon2id$"), "{stored}");
        assert!(!stored.contains(PASSWORD));
    }

    #[tokio::test]
    async fn signing_in_takes_the_right_name_and_password() {
        let house = house().await;
        let (owner, _) = house.owner().await;

        // The name is the same in any case, with spaces around it or not.
        for name in ["Ana", "ana", " ANA "] {
            let (user, session) = house.accounts.sign_in(name, PASSWORD).await.unwrap();
            assert_eq!(user, owner);
            assert_eq!(
                house.accounts.session_user(&session).await.unwrap(),
                Some(owner.clone())
            );
        }
        for (name, password) in [
            ("Ana", "wrong horse"),
            ("Ana", ""),
            ("Ana", &"p".repeat(2000)),
            ("Nobody", PASSWORD),
            ("", PASSWORD),
        ] {
            let refused = house.accounts.sign_in(name, password).await;
            assert!(
                matches!(refused, Err(AccountError::WrongCredentials)),
                "{name:?}"
            );
        }
    }

    #[tokio::test]
    async fn guesses_beyond_a_few_at_once_are_turned_away() {
        let house = house().await;
        house.owner().await;
        let (slow, guesses) = wrong_guesses(&house, Duration::from_millis(300)).await;
        let one_more = slow.sign_in("Ana", PASSWORD).await;
        assert!(matches!(one_more, Err(AccountError::Busy)));

        for guess in guesses {
            let refused = guess.await.unwrap();
            assert!(matches!(refused, Err(AccountError::WrongCredentials)));
        }
        assert!(slow.sign_in("Ana", PASSWORD).await.is_ok());
    }

    type Guess = tokio::task::JoinHandle<Result<(User, String), AccountError>>;

    /// Starts as many wrong guesses as are checked at once, each made to
    /// wait `wait` afterwards, and returns once they hold every place. Also
    /// gives the accounts they were made through.
    async fn wrong_guesses(house: &House, wait: Duration) -> (Accounts, Vec<Guess>) {
        let slow = Accounts {
            failed_wait: wait,
            ..house.accounts.clone()
        };
        let guesses = (0..CHECKS_AT_ONCE)
            .map(|_| {
                let slow = slow.clone();
                tokio::spawn(async move { slow.sign_in("Ana", "wrong horse").await })
            })
            .collect();
        places_free(&slow, 0).await;
        (slow, guesses)
    }

    /// Waits until `free` of the places for password checks are free.
    async fn places_free(accounts: &Accounts, free: usize) {
        for _ in 0..500 {
            if accounts.checks.available_permits() == free {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("{free} places were never free");
    }

    #[tokio::test]
    async fn a_guess_keeps_its_place_when_nobody_waits_for_the_answer() {
        let house = house().await;
        house.owner().await;
        let (slow, guesses) = wrong_guesses(&house, Duration::from_millis(500)).await;

        // Whoever guessed hangs up, as a client does that closes its
        // connection as soon as the answer is not the quick one.
        for guess in guesses {
            guess.abort();
            assert!(guess.await.unwrap_err().is_cancelled());
        }
        let one_more = slow.sign_in("Ana", PASSWORD).await;
        assert!(matches!(one_more, Err(AccountError::Busy)));

        places_free(&slow, CHECKS_AT_ONCE).await;
        assert!(slow.sign_in("Ana", PASSWORD).await.is_ok());
    }

    #[tokio::test]
    async fn a_session_ends_when_signed_out_or_left_unused() {
        let house = house().await;
        let (_, session) = house.owner().await;
        let stored: String = sqlx::query_scalar("SELECT token_hash FROM session")
            .fetch_one(house.db.pool())
            .await
            .unwrap();
        assert_ne!(stored, session);

        assert!(
            house
                .accounts
                .session_user("made up")
                .await
                .unwrap()
                .is_none()
        );

        let (_, second) = house.accounts.sign_in("Ana", PASSWORD).await.unwrap();
        house.accounts.sign_out(&second).await.unwrap();
        assert!(
            house
                .accounts
                .session_user(&second)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            house
                .accounts
                .session_user(&session)
                .await
                .unwrap()
                .is_some()
        );

        sqlx::query("UPDATE session SET last_used_at = last_used_at - ?")
            .bind(SESSION_IDLE + 1)
            .execute(house.db.pool())
            .await
            .unwrap();
        assert!(
            house
                .accounts
                .session_user(&session)
                .await
                .unwrap()
                .is_none()
        );
        // Signing in again clears what is over.
        house.accounts.sign_in("Ana", PASSWORD).await.unwrap();
        assert_eq!(house.count("session").await, 1);
    }

    #[tokio::test]
    async fn using_a_session_is_noted_to_the_minute() {
        let house = house().await;
        let (_, session) = house.owner().await;
        let last_used = || async {
            sqlx::query_scalar::<_, i64>("SELECT last_used_at FROM session")
                .fetch_one(house.db.pool())
                .await
                .unwrap()
        };
        let at_first = last_used().await;
        house.accounts.session_user(&session).await.unwrap();
        assert_eq!(last_used().await, at_first);

        sqlx::query("UPDATE session SET last_used_at = last_used_at - 3600")
            .execute(house.db.pool())
            .await
            .unwrap();
        house.accounts.session_user(&session).await.unwrap();
        assert!(last_used().await >= at_first);
    }

    #[tokio::test]
    async fn a_token_works_until_it_is_revoked() {
        let house = house().await;
        let (owner, _) = house.owner().await;
        let bad_name = house.accounts.create_token(owner.id, "  ").await;
        assert!(matches!(bad_name, Err(AccountError::Invalid(_))));

        let (token, secret) = house
            .accounts
            .create_token(owner.id, " iPad ")
            .await
            .unwrap();
        assert_eq!(token.name, "iPad");
        assert!(token.last_used_at.is_none());
        assert!(secret.starts_with(TOKEN_PREFIX));
        let stored: String = sqlx::query_scalar("SELECT token_hash FROM api_token")
            .fetch_one(house.db.pool())
            .await
            .unwrap();
        assert!(!secret.contains(&stored) && !stored.contains(&secret));

        assert_eq!(
            house.accounts.token_user(&secret).await.unwrap(),
            Some(owner.clone())
        );
        let listed = house.accounts.tokens(owner.id).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].last_used_at.is_some());
        // A token is not a session, and the other way round.
        assert!(
            house
                .accounts
                .session_user(&secret)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            house
                .accounts
                .token_user("binge_made_up")
                .await
                .unwrap()
                .is_none()
        );

        // Nobody revokes someone else's token.
        let other = house
            .accounts
            .create_user("Ben", PASSWORD, &[])
            .await
            .unwrap();
        let not_theirs = house.accounts.revoke_token(other.user.id, token.id).await;
        assert!(matches!(not_theirs, Err(AccountError::NoSuchToken)));
        assert!(
            house
                .accounts
                .tokens(other.user.id)
                .await
                .unwrap()
                .is_empty()
        );

        house
            .accounts
            .revoke_token(owner.id, token.id)
            .await
            .unwrap();
        assert!(house.accounts.token_user(&secret).await.unwrap().is_none());
        let twice = house.accounts.revoke_token(owner.id, token.id).await;
        assert!(matches!(twice, Err(AccountError::NoSuchToken)));
    }

    #[tokio::test]
    async fn the_owner_gets_what_was_read_before_accounts() {
        let house = house().await;
        sqlx::query(
            "INSERT INTO progress_before_accounts (content_hash, page, completed, updated_at) \
             VALUES ('aaa', 4, 0, 100), ('bbb', 9, 1, 200)",
        )
        .execute(house.db.pool())
        .await
        .unwrap();

        let (owner, _) = house.owner().await;
        let theirs: Vec<(UserId, String, i64, bool, i64)> = sqlx::query_as(
            "SELECT user_id, content_hash, page, completed, updated_at FROM progress \
             ORDER BY content_hash",
        )
        .fetch_all(house.db.pool())
        .await
        .unwrap();
        assert_eq!(
            theirs,
            [
                (owner.id, "aaa".to_owned(), 4, false, 100),
                (owner.id, "bbb".to_owned(), 9, true, 200)
            ]
        );
        assert_eq!(house.count("progress_before_accounts").await, 0);
    }

    #[tokio::test]
    async fn users_are_added_with_the_libraries_they_may_read() {
        let house = house().await;
        let (owner, _) = house.owner().await;
        let (comics, manga) = (house.library("Comics").await, house.library("Manga").await);

        let ben = house
            .accounts
            .create_user("Ben", PASSWORD, &[manga, comics, manga])
            .await
            .unwrap();
        assert!(!ben.user.is_owner);
        assert_eq!(ben.libraries, [comics, manga]);
        let (signed_in, _) = house.accounts.sign_in("ben", PASSWORD).await.unwrap();
        assert_eq!(signed_in, ben.user);

        for taken in ["Ben", "BEN", "ana"] {
            let refused = house.accounts.create_user(taken, PASSWORD, &[]).await;
            assert!(matches!(refused, Err(AccountError::NameTaken)), "{taken}");
        }

        // Letters outside A to Z have upper and lower case too.
        let elodie = house
            .accounts
            .create_user("Élodie", PASSWORD, &[])
            .await
            .unwrap();
        assert_eq!(elodie.user.name, "Élodie");
        let (signed_in, _) = house.accounts.sign_in("élodie", PASSWORD).await.unwrap();
        assert_eq!(signed_in, elodie.user);
        let refused = house.accounts.create_user("ÉLODIE", PASSWORD, &[]).await;
        assert!(matches!(refused, Err(AccountError::NameTaken)));
        house.accounts.remove_user(elodie.user.id).await.unwrap();
        let nowhere = house.accounts.create_user("Cy", PASSWORD, &[99]).await;
        assert!(matches!(nowhere, Err(AccountError::NoSuchLibrary)));
        // Nothing is left of the user who could not be added.
        assert_eq!(house.count("user").await, 2);

        let all = house.accounts.accounts().await.unwrap();
        let names: Vec<_> = all.iter().map(|one| one.user.name.as_str()).collect();
        assert_eq!(names, ["Ana", "Ben"]);
        // The owner is listed with every library, given or not.
        assert_eq!(all[0].user, owner);
        assert_eq!(all[0].libraries, [comics, manga]);
        assert_eq!(all[0], house.accounts.account(owner.id).await.unwrap());
        assert_eq!(all[1], ben);

        // A library that is removed takes its permissions with it.
        sqlx::query("DELETE FROM library WHERE id = ?")
            .bind(comics)
            .execute(house.db.pool())
            .await
            .unwrap();
        let ben = house.accounts.account(ben.user.id).await.unwrap();
        assert_eq!(ben.libraries, [manga]);
    }

    #[tokio::test]
    async fn the_owner_changes_and_removes_other_users() {
        let house = house().await;
        let (owner, _) = house.owner().await;
        let comics = house.library("Comics").await;
        let ben = house
            .accounts
            .create_user("Ben", PASSWORD, &[])
            .await
            .unwrap()
            .user;
        let (_, session) = house.accounts.sign_in("Ben", PASSWORD).await.unwrap();
        let (_, token) = house.accounts.create_token(ben.id, "iPad").await.unwrap();

        // Only the libraries: the session goes on.
        let changed = house
            .accounts
            .change_user(ben.id, None, Some(&[comics]))
            .await
            .unwrap();
        assert_eq!(changed.libraries, [comics]);
        assert!(
            house
                .accounts
                .session_user(&session)
                .await
                .unwrap()
                .is_some()
        );

        // A new password: the libraries stay, the sessions end, tokens stay.
        let short = house
            .accounts
            .change_user(ben.id, Some("short"), None)
            .await;
        assert!(matches!(short, Err(AccountError::Invalid(_))));
        let changed = house
            .accounts
            .change_user(ben.id, Some("another horse"), None)
            .await
            .unwrap();
        assert_eq!(changed.libraries, [comics]);
        assert!(
            house
                .accounts
                .session_user(&session)
                .await
                .unwrap()
                .is_none()
        );
        assert!(house.accounts.token_user(&token).await.unwrap().is_some());
        assert!(house.accounts.sign_in("Ben", PASSWORD).await.is_err());
        assert!(house.accounts.sign_in("Ben", "another horse").await.is_ok());

        for refused in [
            house.accounts.change_user(owner.id, None, Some(&[])).await,
            house
                .accounts
                .change_user(owner.id, Some(PASSWORD), None)
                .await,
        ] {
            assert!(matches!(refused, Err(AccountError::IsOwner)));
        }
        let owner_stays = house.accounts.remove_user(owner.id).await;
        assert!(matches!(owner_stays, Err(AccountError::IsOwner)));
        let nobody = house.accounts.remove_user(UserId(99)).await;
        assert!(matches!(nobody, Err(AccountError::NoSuchUser)));

        sqlx::query(
            "INSERT INTO progress (user_id, content_hash, page, updated_at) VALUES (?, 'aaa', 1, 0)",
        )
        .bind(ben.id)
        .execute(house.db.pool())
        .await
        .unwrap();
        house.accounts.remove_user(ben.id).await.unwrap();
        assert!(house.accounts.token_user(&token).await.unwrap().is_none());
        for table in ["progress", "library_access", "api_token"] {
            assert_eq!(house.count(table).await, 0, "{table}");
        }
        assert!(
            house
                .accounts
                .sign_in("Ben", "another horse")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn changing_a_password_takes_the_current_one() {
        let house = house().await;
        let (owner, here) = house.owner().await;
        let (_, elsewhere) = house.accounts.sign_in("Ana", PASSWORD).await.unwrap();
        let (_, token) = house.accounts.create_token(owner.id, "iPad").await.unwrap();
        let accounts = &house.accounts;

        let wrong = accounts
            .change_password(owner.id, "wrong horse", "another horse", Some(&here))
            .await;
        assert!(matches!(wrong, Err(AccountError::WrongPassword)));
        let short = accounts
            .change_password(owner.id, PASSWORD, "short", Some(&here))
            .await;
        assert!(matches!(short, Err(AccountError::Invalid(_))));
        assert!(accounts.session_user(&elsewhere).await.unwrap().is_some());

        accounts
            .change_password(owner.id, PASSWORD, "another horse", Some(&here))
            .await
            .unwrap();
        assert!(accounts.session_user(&here).await.unwrap().is_some());
        assert!(accounts.session_user(&elsewhere).await.unwrap().is_none());
        assert!(accounts.token_user(&token).await.unwrap().is_some());
        assert!(accounts.sign_in("Ana", PASSWORD).await.is_err());
        assert!(accounts.sign_in("Ana", "another horse").await.is_ok());
    }
}
