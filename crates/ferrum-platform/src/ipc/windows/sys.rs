//! The `unsafe` named-pipe calls of the endpoint (`docs/05` §4: one audited module); the token and
//! security-descriptor calls are in [`crate::win32`].

#![allow(unsafe_code, reason = "docs/05 §4: the named-pipe calls, each with SAFETY")]

use std::io;
use std::os::windows::io::FromRawHandle;

use mio::windows::NamedPipe;
use windows_sys::Win32::Foundation::{ERROR_PIPE_BUSY, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT, WaitNamedPipeW,
};

use crate::win32::Security;

/// Pipe buffer sizes in each direction: one daemon read (`docs/02` §5: 64 KiB); larger frames
/// flow through in several reads.
const PIPE_BUFFER_BYTES: u32 = 64 * 1_024;
/// How long a client waits for a free pipe instance per try: the daemon creates the next
/// instance right after accepting one, within a loop turn, so a quarter second is ample.
const WAIT_INSTANCE_MS: u32 = 250;

/// Whether a pipe instance is the first of its name.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Instance {
    /// The first: creation fails if another process owns the name (pipe squatting).
    First,
    /// A further instance of a name this process owns.
    Next,
}

/// Creates one overlapped server instance of the pipe `name` (NUL-terminated).
pub(super) fn create_pipe(
    name: &[u16],
    security: &Security,
    instance: Instance,
) -> io::Result<NamedPipe> {
    let first = if instance == Instance::First { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
    let attributes = security.attributes();
    let mode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
    // SAFETY: `name` is NUL-terminated and `attributes` points to a live descriptor; the result
    // is checked against `INVALID_HANDLE_VALUE`.
    let handle = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | first,
            mode,
            PIPE_UNLIMITED_INSTANCES,
            PIPE_BUFFER_BYTES,
            PIPE_BUFFER_BYTES,
            0,
            &raw const attributes,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a new overlapped pipe handle owned by nobody else.
    Ok(unsafe { NamedPipe::from_raw_handle(handle) })
}

/// Whether opening the pipe failed because every instance is busy.
pub(super) fn is_pipe_busy(error: &io::Error) -> bool {
    error.raw_os_error() == i32::try_from(ERROR_PIPE_BUSY).ok()
}

/// Waits until an instance of the pipe `name` (NUL-terminated) is free, or a short timeout.
pub(super) fn wait_for_instance(name: &[u16]) {
    // SAFETY: `name` is NUL-terminated. The result does not matter: the caller retries the open.
    let _ready = unsafe { WaitNamedPipeW(name.as_ptr(), WAIT_INSTANCE_MS) };
}
