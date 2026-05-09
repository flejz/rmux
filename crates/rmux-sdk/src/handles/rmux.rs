//! Opaque RMUX SDK facade handle.

use std::fmt;
use std::time::Duration;

use super::builder::RmuxBuilder;
use crate::transport::DropGuard;
use crate::{bootstrap::discovery, Result, RmuxEndpoint};

/// Inert SDK facade for daemon-backed RMUX operations.
///
/// Constructing this handle only records endpoint configuration and does not
/// contact a daemon.
pub struct Rmux {
    endpoint: RmuxEndpoint,
    default_timeout: Option<Duration>,
    _drop_guard: DropGuard,
}

impl Rmux {
    /// Creates a facade configured to use default endpoint discovery.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a builder configured to use default endpoint discovery.
    #[must_use]
    pub fn builder() -> RmuxBuilder {
        RmuxBuilder::new()
    }

    /// Returns the endpoint selection recorded by this facade.
    #[must_use]
    pub fn endpoint(&self) -> &RmuxEndpoint {
        &self.endpoint
    }

    /// Returns the operation timeout default recorded by this facade.
    #[must_use]
    pub const fn configured_default_timeout(&self) -> Option<Duration> {
        self.default_timeout
    }

    /// Resolves the endpoint that would be used by runtime SDK operations.
    ///
    /// This consults SDK discovery only when the recorded endpoint is
    /// [`RmuxEndpoint::Default`].
    pub fn resolved_endpoint(&self) -> Result<RmuxEndpoint> {
        discovery::resolve_endpoint(&self.endpoint)
    }

    /// Resolves the timeout that would be used by one runtime SDK operation.
    ///
    /// `per_operation_timeout` has precedence over this facade's configured
    /// default and can use `Duration::MAX` to request no timeout.
    #[must_use]
    pub fn resolved_timeout(&self, per_operation_timeout: Option<Duration>) -> Option<Duration> {
        discovery::resolve_timeout(per_operation_timeout, self.default_timeout)
    }

    pub(crate) fn from_config(endpoint: RmuxEndpoint, default_timeout: Option<Duration>) -> Self {
        Self {
            endpoint,
            default_timeout,
            _drop_guard: DropGuard::noop(),
        }
    }
}

impl Default for Rmux {
    fn default() -> Self {
        RmuxBuilder::default().build()
    }
}

impl fmt::Debug for Rmux {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Rmux").finish_non_exhaustive()
    }
}
