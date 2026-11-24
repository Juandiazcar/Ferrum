//! The login shell of the current user from the user database (`getpwuid_r`), which has no safe
//! wrapper in `rustix` (`docs/05` §4: one audited call).

#![allow(unsafe_code, reason = "docs/05 §4: getpwuid_r has no safe wrapper; SAFETY below")]

use std::ffi::{CStr, OsString};
use std::os::unix::ffi::OsStrExt;

/// Buffer for the strings of one user entry; entries are a few hundred bytes, and a larger one
/// (`ERANGE`) simply yields no shell, after which `/etc/shells` is used.
const ENTRY_BUFFER_BYTES: usize = 4 * 1_024;

/// The login shell of the effective user, when the user database has one.
pub(super) fn login_shell() -> Option<OsString> {
    let uid = rustix::process::geteuid().as_raw();
    // SAFETY: `passwd` is plain data; all-zero is a valid initial value the call overwrites.
    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut buffer = vec![0_u8; ENTRY_BUFFER_BYTES];
    let mut found: *mut libc::passwd = std::ptr::null_mut();
    // SAFETY: every pointer refers to a live local of the expected type; `buffer` is writable for
    // its whole length, which is passed; on success `found` points to `entry`, whose string
    // fields point into `buffer`, both alive until the end of this function.
    let status = unsafe {
        libc::getpwuid_r(
            uid,
            &raw mut entry,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &raw mut found,
        )
    };
    if status != 0 || found.is_null() || entry.pw_shell.is_null() {
        return None;
    }
    // SAFETY: `pw_shell` is non-null and points to a NUL-terminated string inside `buffer`.
    let shell = unsafe { CStr::from_ptr(entry.pw_shell) };
    let shell = std::ffi::OsStr::from_bytes(shell.to_bytes());
    (!shell.is_empty()).then(|| shell.to_os_string())
}
