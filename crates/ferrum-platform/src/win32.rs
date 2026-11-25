//! Every `unsafe` Windows security call of this crate (`docs/05` §4: one audited module): the
//! user of a token, owner-only security descriptors, and the owner/DACL check of a directory
//! (`docs/10` §2: the IPC endpoint and the run directory are the current user's only). Each
//! wrapper owns what it creates and frees it; callers see safe types.

#![allow(unsafe_code, reason = "docs/05 §4: Windows token and ACL calls, each with SAFETY")]

use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;

use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    GetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL, ACL_SIZE_INFORMATION, AclSizeInformation, DACL_SECURITY_INFORMATION,
    EqualSid, GetAce, GetAclInformation, GetTokenInformation, IsWellKnownSid,
    OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, RevertToSelf, SECURITY_ATTRIBUTES,
    TOKEN_QUERY, TOKEN_USER, TokenUser as TOKEN_USER_CLASS, WinLocalSystemSid,
};
use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;
use windows_sys::Win32::System::Pipes::{GetNamedPipeServerProcessId, ImpersonateNamedPipeClient};
use windows_sys::Win32::System::SystemServices::{ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThread, OpenProcess, OpenProcessToken, OpenThreadToken,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

/// A `TOKEN_USER` structure followed by the SID it points to, as `GetTokenInformation` wrote it.
/// Only [`TokenUser::of_token`] builds one, so its SID pointer is always valid; the buffer is
/// never resized, so the pointer stays inside it.
struct TokenUser(Vec<u64>);

impl TokenUser {
    fn of_token(token: &OwnedHandle) -> io::Result<Self> {
        let mut needed = 0;
        // SAFETY: a size query with a null buffer of length 0; it only writes `needed`.
        unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TOKEN_USER_CLASS,
                std::ptr::null_mut(),
                0,
                &raw mut needed,
            )
        };
        let words = usize::try_from(needed).unwrap_or(0).div_ceil(size_of::<u64>());
        let mut buffer = vec![0_u64; words.max(1)];
        let bytes = u32::try_from(buffer.len() * size_of::<u64>()).unwrap_or(u32::MAX);
        let out = buffer.as_mut_ptr().cast::<c_void>();
        // SAFETY: `out` points to `bytes` writable, 8-aligned bytes owned by `buffer`.
        let done = unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TOKEN_USER_CLASS,
                out,
                bytes,
                &raw mut needed,
            )
        };
        if done == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(buffer))
    }

    fn sid(&self) -> PSID {
        // SAFETY: by construction the buffer holds a `TOKEN_USER` written by the OS, large
        // enough and aligned for it (see the type's documentation).
        unsafe { (*self.0.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }
}

/// The user an access token belongs to.
pub(crate) struct User {
    token_user: TokenUser,
}

impl User {
    /// The user of the current process.
    pub(crate) fn current() -> io::Result<Self> {
        // SAFETY: the pseudo handle of the current process needs no closing.
        Self::of_process(unsafe { GetCurrentProcess() })
    }

    fn of_process(process: HANDLE) -> io::Result<Self> {
        let mut raw: HANDLE = std::ptr::null_mut();
        // SAFETY: `process` is a process handle with query access; `raw` receives a new token
        // handle that `OwnedHandle` closes.
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut raw) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a new token handle owned by nobody else.
        let token = unsafe { OwnedHandle::from_raw_handle(raw) };
        Ok(Self { token_user: TokenUser::of_token(&token)? })
    }

    /// The user of the client of a connected server pipe, from the pipe client token
    /// (`docs/10` §2). Windows allows it only after the server read from the pipe.
    pub(crate) fn of_pipe_client(pipe: &impl AsRawHandle) -> io::Result<Self> {
        // SAFETY: `pipe` owns the server end of a connected named pipe, borrowed for the whole call.
        if unsafe { ImpersonateNamedPipeClient(pipe.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut raw: HANDLE = std::ptr::null_mut();
        // SAFETY: the pseudo handle of the current thread; `OpenAsSelf` (1) opens the token with
        // the process's own rights; `raw` receives a new handle closed below.
        let opened = unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &raw mut raw) };
        let error = io::Error::last_os_error();
        // SAFETY: ends the impersonation started above; always called, also after a failure.
        if unsafe { RevertToSelf() } == 0 {
            // The thread would go on with the client's identity: nothing it did after this could
            // be trusted, so the process stops rather than continue as another user.
            std::process::abort();
        }
        if opened == 0 {
            return Err(error);
        }
        // SAFETY: a new token handle owned by nobody else.
        let token = unsafe { OwnedHandle::from_raw_handle(raw) };
        Ok(Self { token_user: TokenUser::of_token(&token)? })
    }

    /// The user of the server process of a connected client pipe.
    pub(crate) fn of_pipe_server(pipe: &impl AsRawHandle) -> io::Result<Self> {
        let mut pid = 0;
        // SAFETY: `pipe` owns a connected client end of a named pipe, borrowed for the whole call.
        if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &raw mut pid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: plain call; a null result is checked below.
        let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a new process handle owned by nobody else.
        let process = unsafe { OwnedHandle::from_raw_handle(raw) };
        Self::of_process(process.as_raw_handle())
    }

    /// Whether both are the same user.
    pub(crate) fn is(&self, other: &Self) -> bool {
        // SAFETY: both SIDs are valid (see `TokenUser`).
        unsafe { EqualSid(self.token_user.sid(), other.token_user.sid()) != 0 }
    }

    /// The SID in its `S-1-5-...` text form.
    pub(crate) fn sid_string(&self) -> io::Result<String> {
        let mut text: *mut u16 = std::ptr::null_mut();
        // SAFETY: the SID is valid (see `TokenUser`); on success `text` is a NUL-terminated
        // string allocated with `LocalAlloc`, freed below.
        if unsafe { ConvertSidToStringSidW(self.token_user.sid(), &raw mut text) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut len = 0;
        // SAFETY: `text` is NUL-terminated, so every read up to the terminator is in bounds.
        while unsafe { *text.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: `text` points to `len` initialized code units.
        let sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) });
        // SAFETY: allocated by `ConvertSidToStringSidW` and not used after this.
        unsafe { LocalFree(text.cast()) };
        Ok(sid)
    }
}

/// A security descriptor from SDDL, freed on drop.
pub(crate) struct Security {
    descriptor: PSECURITY_DESCRIPTOR,
}

impl Security {
    /// Full access for `user` and `SYSTEM` only, protected from inherited entries; with
    /// `inherit`, directories pass the same entries to what is created inside them.
    pub(crate) fn owner_only(user: &User, inherit: Inherit) -> io::Result<Self> {
        let sid = user.sid_string()?;
        let flags = if inherit == Inherit::ToChildren { "OICI" } else { "" };
        // The owner is set as well: an elevated administrator would otherwise make the
        // Administrators group the owner, which `is_private` rightly refuses.
        let sddl = format!("O:{sid}D:P(A;{flags};GA;;;{sid})(A;{flags};GA;;;SY)");
        let sddl: Vec<u16> = sddl.encode_utf16().chain([0]).collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: `sddl` is NUL-terminated; on success `descriptor` is allocated with
        // `LocalAlloc` and freed in `drop`.
        let done = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &raw mut descriptor,
                std::ptr::null_mut(),
            )
        };
        if done == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { descriptor })
    }

    /// Security attributes pointing to this descriptor (not inheritable handles).
    pub(crate) fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
            lpSecurityDescriptor: self.descriptor,
            bInheritHandle: 0,
        }
    }
}

impl Drop for Security {
    fn drop(&mut self) {
        // SAFETY: allocated by `ConvertStringSecurityDescriptorToSecurityDescriptorW`.
        unsafe { LocalFree(self.descriptor) };
    }
}

// SAFETY: the descriptor is an immutable heap block owned by this value and freed only by its
// `drop`, so it may move to another thread with it.
unsafe impl Send for Security {}

/// Whether a descriptor's entries pass to the objects created inside a directory.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Inherit {
    /// Only the object itself.
    No,
    /// Files and directories created inside too.
    ToChildren,
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain([0]).collect()
}

/// Creates one directory that only the current user and `SYSTEM` can access, and whose entries
/// pass to everything created inside it.
pub(crate) fn create_private_dir(path: &Path) -> io::Result<()> {
    let security = Security::owner_only(&User::current()?, Inherit::ToChildren)?;
    let attributes = security.attributes();
    let name = wide(path);
    // SAFETY: `name` is NUL-terminated; `attributes` points to a live descriptor.
    if unsafe { CreateDirectoryW(name.as_ptr(), &raw const attributes) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Whether `path` is owned by the current user and its access list allows nobody else but
/// `SYSTEM`.
pub(crate) fn is_private(path: &Path) -> io::Result<bool> {
    let user = User::current()?;
    let name = wide(path);
    let (mut owner, mut dacl): (PSID, *mut ACL) = (std::ptr::null_mut(), std::ptr::null_mut());
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: `name` is NUL-terminated; on success the out pointers point into `descriptor`,
    // allocated with `LocalAlloc` and freed below after their last use.
    let status = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &raw mut owner,
            std::ptr::null_mut(),
            &raw mut dacl,
            std::ptr::null_mut(),
            &raw mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(i32::try_from(status).unwrap_or(i32::MAX)));
    }
    // SAFETY: `owner` and `dacl` come from the descriptor read above, still allocated.
    let private =
        unsafe { EqualSid(owner, user.token_user.sid()) != 0 && dacl_is_private(dacl, &user) };
    // SAFETY: allocated by `GetNamedSecurityInfoW`; nothing refers to it any more.
    unsafe { LocalFree(descriptor) };
    Ok(private)
}

/// Whether every entry of `dacl` is a deny entry or a plain allow entry naming `user` or `SYSTEM`
/// (a null DACL allows everyone; any other entry type is refused).
///
/// # Safety
/// `dacl` must be null or point to a valid ACL that stays allocated during the call.
unsafe fn dacl_is_private(dacl: *mut ACL, user: &User) -> bool {
    if dacl.is_null() {
        return false;
    }
    let mut info = ACL_SIZE_INFORMATION { AceCount: 0, AclBytesInUse: 0, AclBytesFree: 0 };
    let size = u32::try_from(size_of::<ACL_SIZE_INFORMATION>()).unwrap_or(u32::MAX);
    // SAFETY: `dacl` is a valid ACL (the caller's contract); `info` is writable for `size`.
    if unsafe { GetAclInformation(dacl, (&raw mut info).cast(), size, AclSizeInformation) } == 0 {
        return false;
    }
    (0..info.AceCount).all(|index| {
        let mut ace: *mut c_void = std::ptr::null_mut();
        // SAFETY: `index` is below the ACL's entry count; `ace` receives a pointer into it.
        if unsafe { GetAce(dacl, index, &raw mut ace) } == 0 {
            return false;
        }
        let ace = ace.cast::<ACCESS_ALLOWED_ACE>();
        // SAFETY: every ACE starts with its header; an allow entry is an `ACCESS_ALLOWED_ACE`
        // whose SID starts at `SidStart`.
        unsafe {
            match u32::from((*ace).Header.AceType) {
                ACCESS_ALLOWED_ACE_TYPE => {}
                // Deny entries cannot open the directory to anyone.
                ACCESS_DENIED_ACE_TYPE => return true,
                // Object, callback and compound allow entries, and any other type, are not
                // checked here: the directory is not considered private.
                _ => return false,
            }
            let sid: PSID = (&raw mut (*ace).SidStart).cast();
            EqualSid(sid, user.token_user.sid()) != 0 || IsWellKnownSid(sid, WinLocalSystemSid) != 0
        }
    })
}
