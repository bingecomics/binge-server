//! What tells one build of the server from another of the same version.
//!
//! The build scripts say it (`scripts/build-mark.sh`). A plain `cargo build`
//! does not, and the server then gives only its version.

/// The commit the server was built from, with a `+` after it when files
/// differed from that commit.
pub fn commit() -> Option<&'static str> {
    option_env!("BINGE_BUILD_COMMIT").filter(|commit| !commit.is_empty())
}

/// When the server was built, in seconds since 1970.
pub fn at() -> Option<i64> {
    option_env!("BINGE_BUILD_TIME")?.parse().ok()
}
