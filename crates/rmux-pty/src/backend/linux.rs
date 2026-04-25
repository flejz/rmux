use std::os::fd::{BorrowedFd, OwnedFd, RawFd};

use rustix::process::{getpid, ioctl_tiocsctty, kill_process_group, setsid, Pid};
use rustix::pty::{grantpt, ioctl_tiocgptpeer, openpt, unlockpt, OpenptFlags};
use rustix::termios::{tcgetwinsize, tcsetpgrp, tcsetwinsize};

use crate::{Result, Signal, TerminalSize};

pub(crate) fn open_pty_pair() -> Result<(OwnedFd, OwnedFd)> {
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC)?;
    grantpt(&master)?;
    unlockpt(&master)?;

    let slave = ioctl_tiocgptpeer(
        &master,
        OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC,
    )?;

    Ok((master, slave))
}

pub(crate) fn query_size(fd: BorrowedFd<'_>) -> Result<TerminalSize> {
    Ok(TerminalSize::from_winsize(tcgetwinsize(fd)?))
}

pub(crate) fn apply_size(fd: BorrowedFd<'_>, size: TerminalSize) -> Result<()> {
    tcsetwinsize(fd, size.into_winsize())?;
    Ok(())
}

pub(crate) fn setup_child_controlling_terminal(raw_master_fd: RawFd) -> std::io::Result<()> {
    // SAFETY: This closes only the child process' inherited copy of the PTY
    // master fd. The parent still owns its separate descriptor.
    unsafe { rustix::io::close(raw_master_fd) };

    setsid().map_err(std::io::Error::from)?;

    // SAFETY: `stdin` has already been wired to the PTY slave by `Command`, so
    // fd 0 is a valid borrowed descriptor for the rest of the pre-exec setup.
    let slave_stdin = unsafe { BorrowedFd::borrow_raw(0) };
    ioctl_tiocsctty(slave_stdin).map_err(std::io::Error::from)?;
    tcsetpgrp(slave_stdin, getpid()).map_err(std::io::Error::from)?;

    Ok(())
}

pub(crate) fn kill_foreground_process_group(pid: Pid, signal: Signal) -> Result<()> {
    kill_process_group(pid, signal)?;
    Ok(())
}
