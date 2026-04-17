use std::fs;
use std::os::fd::BorrowedFd;
use std::path::Path;

use rustix::termios::tcgetpgrp;

use super::RuntimeFormatContext;

impl RuntimeFormatContext<'_> {
    pub(super) fn pane_foreground_pid(&self) -> Option<u32> {
        let session_name = self.session_name()?;
        let window_index = self.window_index?;
        let pane = self.pane?;
        let state = self.state?;
        state
            .pane_master_fd(session_name, window_index, pane.index())
            .ok()
            .and_then(process_foreground_pid)
            .or_else(|| {
                state
                    .pane_pid_in_window(session_name, window_index, pane.index())
                    .ok()
            })
    }

    pub(super) fn pane_current_path(&self) -> Option<String> {
        let pid = self.pane_foreground_pid()?;
        process_current_path(pid).or_else(|| self.pane_screen_path())
    }

    pub(super) fn pane_current_command(&self) -> Option<String> {
        let state = self.state?;
        let pid = self.pane_foreground_pid()?;
        process_command_name(pid).or_else(|| {
            let session_name = self.session_name()?;
            let window_index = self.window_index?;
            let pane = self.pane?;
            state
                .pane_profile_in_window(session_name, window_index, pane.index())
                .ok()
                .and_then(|profile| {
                    profile
                        .shell()
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(str::to_owned)
                })
        })
    }
}

fn process_command_name(pid: u32) -> Option<String> {
    process_command_name_from_cmdline(pid).or_else(|| process_command_name_from_comm(pid))
}

fn process_foreground_pid(fd: BorrowedFd<'_>) -> Option<u32> {
    let pgrp = tcgetpgrp(fd).ok()?;
    u32::try_from(pgrp.as_raw_nonzero().get()).ok()
}

fn process_current_path(pid: u32) -> Option<String> {
    fs::read_link(format!("/proc/{pid}/cwd"))
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

fn process_command_name_from_cmdline(pid: u32) -> Option<String> {
    let cmdline = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let first = cmdline
        .split(|byte| *byte == 0)
        .find(|segment| !segment.is_empty())?;
    executable_name(std::str::from_utf8(first).ok()?)
}

fn process_command_name_from_comm(pid: u32) -> Option<String> {
    let comm = fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    executable_name(comm.trim())
}

fn executable_name(path: &str) -> Option<String> {
    let name = Path::new(path).file_name()?.to_string_lossy();
    let trimmed = name.trim_start_matches('-');
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}
