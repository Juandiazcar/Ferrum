//! The client end of the named pipe, opened for overlapped I/O so that one thread can block in a
//! read while another writes: on a synchronous handle Windows serializes the two on the shared
//! file object, and a reader waiting for the daemon's answer would block the request it answers.
//! Each direction waits for its own operation with its own event, so the calls stay blocking for
//! the caller.

#![allow(unsafe_code, reason = "docs/05 §4: overlapped ReadFile/WriteFile, each with SAFETY")]

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::Arc;

use windows_sys::Win32::Foundation::{
    ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_PIPE_NOT_CONNECTED,
};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows_sys::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Threading::CreateEventW;

/// One direction of an overlapped pipe handle shared by both directions.
pub(super) struct Half {
    pipe: Arc<std::fs::File>,
    event: OwnedHandle,
}

impl Half {
    /// A direction of `pipe`, which must have been opened with `FILE_FLAG_OVERLAPPED`.
    pub(super) fn new(pipe: Arc<std::fs::File>) -> io::Result<Self> {
        // SAFETY: plain call creating a manual-reset, initially unsignaled, unnamed event; the
        // result is checked for null.
        let raw = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a new event handle owned by nobody else.
        let event = unsafe { OwnedHandle::from_raw_handle(raw) };
        Ok(Self { pipe, event })
    }

    /// Starts one operation with `start` and waits for it; the end of the pipe reads as 0 bytes.
    fn complete(&self, start: impl FnOnce(*mut OVERLAPPED) -> i32) -> io::Result<usize> {
        // SAFETY: OVERLAPPED is plain data; all-zero is its documented initial state.
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = self.event.as_raw_handle();
        let mut done = 0;
        let handle = self.pipe.as_raw_handle();
        if start(&raw mut overlapped) == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != i32::try_from(ERROR_IO_PENDING).ok() {
                return ended(error);
            }
        }
        // SAFETY: `overlapped` lives on this frame until the operation completes (the call waits,
        // `bWait` = TRUE), and `handle` is the handle the operation was started on.
        if unsafe { GetOverlappedResult(handle, &raw const overlapped, &raw mut done, 1) } == 0 {
            return ended(io::Error::last_os_error());
        }
        Ok(usize::try_from(done).unwrap_or(0))
    }
}

/// A closed pipe is the end of the stream, not an error.
fn ended(error: io::Error) -> io::Result<usize> {
    let closed = [ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED];
    if closed.iter().any(|&code| error.raw_os_error() == i32::try_from(code).ok()) {
        return Ok(0);
    }
    Err(error)
}

fn len(buf_len: usize) -> u32 {
    u32::try_from(buf_len).unwrap_or(u32::MAX)
}

impl io::Read for Half {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let handle = self.pipe.as_raw_handle();
        let (ptr, count) = (buf.as_mut_ptr(), len(buf.len()));
        // SAFETY: `ptr` is valid for `count` writable bytes for the whole operation, which
        // `complete` waits for before `buf` can be used again.
        self.complete(|overlapped| unsafe {
            ReadFile(handle, ptr, count, std::ptr::null_mut(), overlapped)
        })
    }
}

impl io::Write for Half {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let handle = self.pipe.as_raw_handle();
        let (ptr, count) = (buf.as_ptr(), len(buf.len()));
        // SAFETY: `ptr` is valid for `count` readable bytes for the whole operation, which
        // `complete` waits for.
        let written = self.complete(|overlapped| unsafe {
            WriteFile(handle, ptr, count, std::ptr::null_mut(), overlapped)
        })?;
        if written == 0 && !buf.is_empty() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
