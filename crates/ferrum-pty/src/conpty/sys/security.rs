//! An owner-only security descriptor for the output pipes (`docs/10` §2: owner-only IPC): a
//! protected access list that grants access to the user this process runs as and to nobody
//! else, built from the user's SID read from the process token.

#![allow(unsafe_code, reason = "docs/05 §4 and ADR 0026: Windows security calls, each with SAFETY")]

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::check;

/// A security descriptor whose access list allows only the current user; freed on drop.
pub(in crate::conpty) struct OwnerOnly(PSECURITY_DESCRIPTOR);

impl OwnerOnly {
    pub(in crate::conpty) fn new() -> io::Result<Self> {
        // `D:P(A;;GA;;;<sid>)`: a protected DACL with one entry, all access for the user.
        let sddl = format!("D:P(A;;GA;;;{})", current_user_sid()?);
        let sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: `sddl` is NUL-terminated and outlives the call; `descriptor` is a valid out
        // pointer, set to a `LocalAlloc` block on success that `drop` frees.
        check(unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &raw mut descriptor,
                std::ptr::null_mut(),
            )
        })?;
        Ok(Self(descriptor))
    }

    /// Attributes that apply this descriptor to a new object, not inheritable. They point into
    /// `self`, which must outlive the call that uses them.
    pub(in crate::conpty) fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
            lpSecurityDescriptor: self.0,
            bInheritHandle: 0,
        }
    }
}

impl Drop for OwnerOnly {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from `ConvertStringSecurityDescriptorToSecurityDescriptorW`,
        // which allocates with `LocalAlloc`, and is freed once. A failure leaks a few bytes.
        let _leaked_on_failure = unsafe { LocalFree(self.0) };
    }
}

/// Longest SID string Windows produces (`SECURITY_MAX_SID_STRING_CHARACTERS`).
const MAX_SID_STRING_UNITS: usize = 187;

/// The SID of the user this process runs as, in its string form (`S-1-5-21-...`).
fn current_user_sid() -> io::Result<String> {
    let mut token = std::ptr::null_mut();
    // SAFETY: the pseudo handle of the current process is always valid; `token` is a valid out
    // pointer whose handle `OwnedHandle` closes.
    check(unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) })?;
    // SAFETY: on success `token` is an open handle that nothing else owns.
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut bytes = 0u32;
    // SAFETY: a null buffer of length 0 is the documented size query; it fails with
    // `ERROR_INSUFFICIENT_BUFFER` and writes the size needed.
    let _size_query = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            std::ptr::null_mut(),
            0,
            &raw mut bytes,
        )
    };
    // `u64` units keep the buffer aligned for `TOKEN_USER`.
    let mut buffer = vec![0u64; usize::try_from(bytes).unwrap_or(0).div_ceil(size_of::<u64>())];
    // SAFETY: `buffer` is writable for at least `bytes` bytes.
    check(unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            bytes,
            &raw mut bytes,
        )
    })?;
    // SAFETY: the call succeeded, so the buffer starts with a `TOKEN_USER`; it is 8-aligned.
    let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut text = std::ptr::null_mut();
    // SAFETY: `sid` points into `buffer`, alive here; `text` is a valid out pointer, set to a
    // `LocalAlloc` NUL-terminated string on success.
    check(unsafe { ConvertSidToStringSidW(sid, &raw mut text) })?;
    // SAFETY: `text` is NUL-terminated (checked one unit at a time) and freed once below.
    let string = unsafe {
        let length = (0..MAX_SID_STRING_UNITS).take_while(|&index| *text.add(index) != 0).count();
        let string = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
        let _leaked_on_failure = LocalFree(text.cast());
        string
    };
    Ok(string)
}
