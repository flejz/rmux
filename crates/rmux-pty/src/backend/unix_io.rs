use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};

pub(crate) fn read(fd: BorrowedFd<'_>, buffer: &mut [u8]) -> io::Result<usize> {
    rustix::io::read(fd, buffer).map_err(io::Error::from)
}

pub(crate) fn write_all(fd: BorrowedFd<'_>, mut buffer: &[u8]) -> io::Result<()> {
    while !buffer.is_empty() {
        match rustix::io::write(fd, buffer) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "write returned 0")),
            Ok(bytes_written) => buffer = &buffer[bytes_written..],
            Err(rustix::io::Errno::INTR) => continue,
            Err(rustix::io::Errno::AGAIN) => wait_until_writable(fd)?,
            Err(error) => return Err(error.into()),
        }
    }

    Ok(())
}

fn wait_until_writable(fd: BorrowedFd<'_>) -> io::Result<()> {
    loop {
        let mut poll_fd = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLOUT,
            revents: 0,
        };
        // SAFETY: `poll_fd` points to one initialized pollfd entry and the
        // borrowed fd stays valid for the duration of this blocking call.
        let ready = unsafe { libc::poll(&mut poll_fd, 1, -1) };
        if ready > 0 {
            if poll_fd.revents & libc::POLLOUT != 0 {
                return Ok(());
            }
            if poll_fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "pty is no longer writable",
                ));
            }
            continue;
        }

        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(error);
    }
}
