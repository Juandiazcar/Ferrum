//! `ferrum-config`: settings model, `TOML` persistence, schema versions, migrations and the
//! export/import archive (`docs/02` §8).
//!
//! # Model
//!
//! - [`Settings`] is the typed model of `settings.toml`; every field has a default and range
//!   checks run while deserializing.
//! - [`Schema`] versions a file kind. Files carry a top-level `schema_version`; old files are
//!   migrated in memory by pure [`Migration`] steps over the `TOML` table before the typed value is
//!   built ([`SETTINGS_SCHEMA`] goes from the synthetic version 0 to version 1).
//! - [`store`] loads (capped at [`store::MAX_FILE_BYTES`], unknown keys ignored, corrupt files
//!   set aside as `*.bad-<timestamp>`) and saves atomically (temporary file, `fsync`, rename).
//!
//! Paths are always arguments: the per-user directories come from `ferrum-platform` and are
//! wired by the composition roots.
//!
//! Allowed internal dependencies: none (foundation).
//! Must never depend on: any other ferrum crate, OS-specific code.

#![forbid(unsafe_code)]

mod error;
mod schema;
mod settings;
pub mod store;
mod value;

pub use error::ConfigError;
pub use schema::{Migrated, Migration, Schema, VERSION_KEY};
pub use settings::*;
pub use store::{Loaded, Source};
pub use value::Rgb;
