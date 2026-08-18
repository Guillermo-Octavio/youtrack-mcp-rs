//! YouTrack REST API domain models.
//!
//! Every type here is deserialized straight from a YouTrack REST API JSON
//! response. Field sets returned by YouTrack vary by instance version,
//! permissions, and the `fields` projection a request asked for, so every
//! struct here deserializes leniently: `#[serde(default)]` at the
//! container level fills in any field YouTrack omitted, rather than
//! failing the whole response.

pub mod agile;
pub mod issue;
pub mod project;
