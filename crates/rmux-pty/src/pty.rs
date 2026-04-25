use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use crate::{backend, Result, TerminalSize};

/// The master endpoint of a pseudoterminal.
#[derive(Debug)]
pub struct PtyMaster {
    fd: OwnedFd,
}

impl PtyMaster {
    /// Queries the current terminal geometry for this PTY.
    pub fn size(&self) -> Result<TerminalSize> {
        backend::query_size(self.as_fd())
    }

    /// Resizes this PTY.
    pub fn resize(&self, size: TerminalSize) -> Result<()> {
        backend::apply_size(self.as_fd(), size)
    }

    /// Duplicates the master file descriptor.
    pub fn try_clone(&self) -> Result<Self> {
        Ok(Self {
            fd: self.fd.try_clone()?,
        })
    }

    /// Consumes the wrapper and returns the owned file descriptor.
    #[must_use]
    pub fn into_owned_fd(self) -> OwnedFd {
        self.fd
    }
}

impl AsFd for PtyMaster {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

/// The slave endpoint of a pseudoterminal.
#[derive(Debug)]
pub struct PtySlave {
    fd: OwnedFd,
}

impl PtySlave {
    /// Queries the current terminal geometry for this PTY.
    pub fn size(&self) -> Result<TerminalSize> {
        backend::query_size(self.as_fd())
    }

    /// Duplicates the slave file descriptor.
    pub fn try_clone(&self) -> Result<Self> {
        Ok(Self {
            fd: self.fd.try_clone()?,
        })
    }

    /// Consumes the wrapper and returns the owned file descriptor.
    #[must_use]
    pub fn into_owned_fd(self) -> OwnedFd {
        self.fd
    }
}

impl AsFd for PtySlave {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

/// A freshly allocated PTY master/slave pair.
#[derive(Debug)]
pub struct PtyPair {
    master: PtyMaster,
    slave: PtySlave,
}

impl PtyPair {
    /// Allocates a PTY pair using the platform backend.
    pub fn open() -> Result<Self> {
        let (master, slave) = backend::open_pty_pair()?;

        Ok(Self {
            master: PtyMaster { fd: master },
            slave: PtySlave { fd: slave },
        })
    }

    /// Allocates a PTY pair and applies an initial window size.
    pub fn open_with_size(size: TerminalSize) -> Result<Self> {
        let pair = Self::open()?;
        pair.master.resize(size)?;
        Ok(pair)
    }

    /// Returns the master endpoint.
    #[must_use]
    pub fn master(&self) -> &PtyMaster {
        &self.master
    }

    /// Returns the slave endpoint.
    #[must_use]
    pub fn slave(&self) -> &PtySlave {
        &self.slave
    }

    /// Consumes the pair and returns the two endpoints.
    #[must_use]
    pub fn into_split(self) -> (PtyMaster, PtySlave) {
        (self.master, self.slave)
    }
}
