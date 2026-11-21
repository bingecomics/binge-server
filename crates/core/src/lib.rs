//! Binge's domain logic: storage, accounts, libraries, scanning, reading and
//! progress.
//!
//! Nothing in this crate knows about HTTP, so the web API and the future MCP
//! server can both sit on top of it.

pub mod accounts;
mod archive;
pub mod backup;
pub mod catalog;
pub mod cbl;
pub mod changes;
pub mod collections;
pub mod comicinfo;
mod covers;
pub mod db;
pub mod favorites;
pub mod from_files;
pub mod history;
mod import;
pub mod layout;
pub mod lists;
pub mod narrow;
mod natural;
pub mod overview;
pub mod people;
pub mod reading;
pub mod scan;
pub mod search;
mod settings;
pub mod sharing;
mod thumbnail;
pub mod wanted;

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;

pub use accounts::{Account, AccountError, Accounts, ApiToken, User, UserId, UserName};
pub use backup::{BackupError, Backups, COPIES_KEPT, DatabaseCopy};
pub use catalog::{
    Book, Catalog, CatalogError, Group, GroupKind, HeldFile, Library, PartOf, Series, Span,
};
pub use cbl::{CblEntry, CblFile, CblFound, CblHow, CblLine, CblPlan, CblRuleFor};
pub use changes::{ChangeKind, ScanChange, ScanRecord};
pub use collections::Collection;
pub use comicinfo::ComicInfo;
pub use covers::{CoverFrom, CoverOf};
pub use db::{Database, DbError};
pub use favorites::FavoritePage;
pub use from_files::FromFiles;
pub use history::{ReadWriter, ReadingDay, ReadingMonth, ReadingNow, ReadingYear};
pub use layout::Place;
pub use lists::{ReadingList, ReadingListOverview};
pub use narrow::{
    Choice, Choices, Letter, Named, NamedPerson, Narrowing, ReadState, SeriesOrder, SeriesSort,
    Years,
};
pub use overview::{
    After, BooksOverview, Followed, GroupOverview, ReadSoFar, SeriesOverview, Summary,
};
pub use people::{Credited, Credits, Person, PersonName};
pub use reading::{BooksToMark, Page, PageImage, Progress};
pub use scan::{ScanPhase, ScanStatus};
pub use search::Found;
pub use sharing::Sharing;

/// What to tell a person when the server failed in a way that is not theirs
/// to fix. What went wrong goes to the log.
pub const FAILED_FOR_PEOPLE: &str = "The server could not do that. Its log has the details.";

/// Version shared by every crate in the workspace.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
