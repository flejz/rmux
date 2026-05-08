#![deny(missing_docs)]
#![forbid(unsafe_code)]

//! Public daemon-backed RMUX SDK scaffolding.
//!
//! v1 introduces a fully daemon-backed public SDK. This crate currently
//! exposes only the compile-time vocabulary and facade-error skeletons
//! needed by later steps; daemon transport, handle types, and event
//! plumbing land in subsequent commits.
//!
//! `rmux-sdk` is a public integration peer of `rmux-client` and must not
//! depend on `rmux-client`, `rmux-core`, `rmux-server`, or `rmux-pty` as
//! normal dependencies. The authoritative identity newtypes
//! (`SessionName`, `SessionId`, `WindowId`, `PaneId`) live in
//! `rmux-proto` and are re-exported here so SDK users import them through
//! `rmux_sdk` without ever depending on those internal crates.

pub mod error;
pub mod types;

pub use error::{Result, RmuxError};
pub use types::{PaneId, RmuxEndpoint, SessionId, SessionName, WindowId};
