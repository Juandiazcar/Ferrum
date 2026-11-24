//! Every `unsafe` call of the `ConPTY` backend (`docs/05` §4: one audited module). Each wrapper
//! owns what it creates and releases it on drop, so callers only see safe types.

#![allow(unsafe_code, reason = "docs/05 §4 and ADR 0026: the ConPTY calls, each with SAFETY")]

use std::ffi::c_void;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};

use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Console::{
    COORD, ClosePseudoConsole, CreatePseudoConsole, HPCON, PSEUDOCONSOLE_INHERIT_CURSOR,
    ResizePseudoConsole,
};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::System::Threading::{
    CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, InitializeProcThreadAttributeList,
    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    STARTUPINFOW, TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject,
};

use crate::PtySize;

mod security;

/// Pseudoconsole flags: `PSEUDOCONSOLE_INHERIT_CURSOR`, the configuration measured by spike S4
/// (`docs/11` §4: the only startup output is the cursor query, then no repaint).
const PSEUDOCONSOLE_FLAGS: u32 = PSEUDOCONSOLE_INHERIT_CURSOR;

/// Exit code given to a child ended by `kill` (`TerminateProcess` requires one).
const KILLED_EXIT_CODE: u32 = 1;

fn coord(size: PtySize) -> COORD {
    // `COORD` holds `i16`; a size above `i16::MAX` cells is clamped (no screen is that large).
    let clamp = |value: u16| i16::try_from(value).unwrap_or(i16::MAX);
    COORD { X: clamp(size.cols.get()), Y: clamp(size.rows.get()) }
}

/// `FACILITY_WIN32`: an `HRESULT` that wraps a Win32 error code in its low 16 bits.
const FACILITY_WIN32: u32 = 7;
/// Bits 16 to 26 of an `HRESULT` hold its facility.
const FACILITY_SHIFT: u32 = 16;
const FACILITY_MASK: u32 = 0x7FF;
/// The Win32 error code inside a `FACILITY_WIN32` `HRESULT`.
const WIN32_CODE_MASK: u32 = 0xFFFF;

/// A failed `HRESULT` as an `io::Error`: the wrapped Win32 error when it carries one.
fn hresult(code: i32) -> io::Result<()> {
    if code >= 0 {
        return Ok(());
    }
    let bits = code.cast_unsigned();
    if (bits >> FACILITY_SHIFT) & FACILITY_MASK == FACILITY_WIN32 {
        let win32 = i32::try_from(bits & WIN32_CODE_MASK).unwrap_or(i32::MAX);
        return Err(io::Error::from_raw_os_error(win32));
    }
    Err(io::Error::other(format!("HRESULT {bits:#010x}")))
}

pub(super) fn check(success: i32) -> io::Result<()> {
    if success != 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

/// A pseudoconsole; dropping it closes it, which ends every process attached to it.
pub(super) struct PseudoConsole(HPCON);

impl PseudoConsole {
    /// Creates a pseudoconsole that reads input from `input` and writes output to `output`. The
    /// console duplicates both handles, so the caller closes its copies afterwards.
    pub(super) fn new(
        size: PtySize,
        input: &impl AsRawHandle,
        output: &impl AsRawHandle,
    ) -> io::Result<Self> {
        let mut console: HPCON = 0;
        // SAFETY: both handles are open pipe ends borrowed for the call, and `console` is a valid
        // out pointer; on success the console owns duplicates of the handles.
        let code = unsafe {
            CreatePseudoConsole(
                coord(size),
                input.as_raw_handle(),
                output.as_raw_handle(),
                PSEUDOCONSOLE_FLAGS,
                &raw mut console,
            )
        };
        hresult(code).map(|()| Self(console))
    }

    pub(super) fn resize(&self, size: PtySize) -> io::Result<()> {
        // SAFETY: `self.0` is a live pseudoconsole: it is closed only in `drop`.
        hresult(unsafe { ResizePseudoConsole(self.0, coord(size)) })
    }

    pub(super) fn handle(&self) -> HPCON {
        self.0
    }
}

impl Drop for PseudoConsole {
    fn drop(&mut self) {
        // SAFETY: `self.0` is a live pseudoconsole and this is its only close.
        unsafe { ClosePseudoConsole(self.0) }
    }
}

/// A process attribute list holding one attribute: the pseudoconsole to attach.
struct AttributeList {
    /// `u64` units keep the list 8-byte aligned, as the OS expects.
    storage: Vec<u64>,
}

impl AttributeList {
    fn with_console(console: &PseudoConsole) -> io::Result<Self> {
        let mut bytes = 0usize;
        // SAFETY: a null list with a valid size pointer is the documented size query; it fails
        // with `ERROR_INSUFFICIENT_BUFFER` and writes the required size.
        let _size_query = unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &raw mut bytes)
        };
        let mut storage = vec![0u64; bytes.div_ceil(size_of::<u64>())];
        // SAFETY: the storage has at least `bytes` bytes, aligned to 8.
        check(unsafe {
            InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 1, 0, &raw mut bytes)
        })?;
        // Only an initialized list is wrapped, so `drop` never deletes an uninitialized one.
        let mut list = Self { storage };
        // SAFETY: the list was initialized for one attribute; the pseudoconsole attribute takes the
        // `HPCON` value itself as the payload, and `console` outlives the list.
        check(unsafe {
            UpdateProcThreadAttribute(
                list.raw(),
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                console.handle() as *const c_void,
                size_of::<HPCON>(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        })
        .map(|()| list)
    }

    fn raw(&mut self) -> *mut c_void {
        self.storage.as_mut_ptr().cast()
    }
}

impl Drop for AttributeList {
    fn drop(&mut self) {
        // SAFETY: only an initialized list is ever wrapped (see `with_console`); deleted once.
        unsafe { DeleteProcThreadAttributeList(self.raw()) }
    }
}

/// What `CreateProcessW` needs, all NUL-terminated UTF-16.
pub(super) struct Launch<'a> {
    pub(super) application: &'a [u16],
    pub(super) command_line: &'a mut [u16],
    pub(super) environment: &'a [u16],
    pub(super) cwd: &'a [u16],
}

/// A child process; the handle stays open until the last owner drops it.
pub(super) struct Process {
    handle: OwnedHandle,
    pid: u32,
}

/// Starts `launch` attached to `console`, inheriting no handle.
pub(super) fn spawn(console: &PseudoConsole, launch: &mut Launch<'_>) -> io::Result<Process> {
    let mut attributes = AttributeList::with_console(console)?;
    let startup = STARTUPINFOEXW {
        StartupInfo: STARTUPINFOW {
            cb: u32::try_from(size_of::<STARTUPINFOEXW>()).unwrap_or(u32::MAX),
            // Invalid standard handles: the child must use the pseudoconsole, never handles the
            // daemon's own standard streams may have been redirected to.
            dwFlags: STARTF_USESTDHANDLES,
            hStdInput: INVALID_HANDLE_VALUE,
            hStdOutput: INVALID_HANDLE_VALUE,
            hStdError: INVALID_HANDLE_VALUE,
            ..STARTUPINFOW::default()
        },
        lpAttributeList: attributes.raw(),
    };
    let mut info = PROCESS_INFORMATION::default();
    // SAFETY: every string is NUL-terminated (built by the caller), the command line buffer is
    // writable as `CreateProcessW` requires, the environment block matches
    // `CREATE_UNICODE_ENVIRONMENT`, `startup` and its attribute list outlive the call, and
    // `bInheritHandles` is false so no handle leaks into the child.
    check(unsafe {
        CreateProcessW(
            launch.application.as_ptr(),
            launch.command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            launch.environment.as_ptr().cast(),
            launch.cwd.as_ptr(),
            (&raw const startup).cast(),
            &raw mut info,
        )
    })?;
    // SAFETY: on success both handles are open and owned by this process; wrapping them hands
    // the close to `OwnedHandle`. The thread handle is not needed and closes here.
    let (handle, _thread) = unsafe {
        (OwnedHandle::from_raw_handle(info.hProcess), OwnedHandle::from_raw_handle(info.hThread))
    };
    Ok(Process { handle, pid: info.dwProcessId })
}

impl Process {
    pub(super) fn pid(&self) -> u32 {
        self.pid
    }

    pub(super) fn raw_handle(&self) -> RawHandle {
        self.handle.as_raw_handle()
    }

    /// The exit code once the process has exited. Asks the process handle whether it is
    /// signaled, so an exit code of 259 (`STILL_ACTIVE`) is reported like any other (W2).
    pub(super) fn exit_code(&self) -> io::Result<Option<u32>> {
        // SAFETY: the handle is open for the lifetime of `self`; a zero timeout never blocks.
        match unsafe { WaitForSingleObject(self.raw_handle(), 0) } {
            WAIT_OBJECT_0 => {
                let mut code = 0u32;
                // SAFETY: the handle is open and `code` is a valid out pointer.
                check(unsafe { GetExitCodeProcess(self.raw_handle(), &raw mut code) })?;
                Ok(Some(code))
            }
            WAIT_TIMEOUT => Ok(None),
            _ => Err(io::Error::last_os_error()),
        }
    }

    /// Ends the process. Succeeds when the process is already gone or exiting (W2: truthful,
    /// idempotent): `TerminateProcess` refuses with `ERROR_ACCESS_DENIED` a process that has begun
    /// to exit, and the handle from `CreateProcessW` has every access right, so that refusal can
    /// only mean the process is ending.
    pub(super) fn terminate(&self) -> io::Result<Terminated> {
        // SAFETY: the handle is open for the lifetime of `self`.
        let result = check(unsafe { TerminateProcess(self.raw_handle(), KILLED_EXIT_CODE) });
        match result {
            Ok(()) => Ok(Terminated::Now),
            Err(error) if error.raw_os_error() == i32::try_from(ERROR_ACCESS_DENIED).ok() => {
                Ok(Terminated::AlreadyEnding)
            }
            Err(_) if self.exit_code()?.is_some() => Ok(Terminated::AlreadyEnding),
            Err(error) => Err(error),
        }
    }
}

/// What [`Process::terminate`] did.
pub(super) enum Terminated {
    /// This call ended the process.
    Now,
    /// The process had already exited or was exiting on its own.
    AlreadyEnding,
}

/// Longest system directory path `GetSystemDirectoryW` is asked for (`MAX_PATH`).
const SYSTEM_DIRECTORY_UNITS: usize = 260;

/// The Windows system directory (`C:\Windows\System32`), where `cmd.exe` lives; read from the OS,
/// not from the environment, so a changed `SystemRoot` cannot redirect batch files.
pub(super) fn system_directory() -> io::Result<std::path::PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    let mut buffer = [0u16; SYSTEM_DIRECTORY_UNITS];
    let capacity = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
    // SAFETY: `buffer` is writable for `capacity` units; the call writes at most that many.
    let written = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), capacity) };
    let units = buffer.get(..usize::try_from(written).unwrap_or(usize::MAX));
    match units {
        Some(units) if written != 0 => Ok(std::ffi::OsString::from_wide(units).into()),
        _ => Err(io::Error::last_os_error()),
    }
}

/// Kernel buffer of the output pipe: one chunk of the daemon's read (`docs/02` §5: at most 64 KiB).
const PIPE_BUFFER_BYTES: u32 = 64 * 1024;

/// Number of output pipes created by this process, which makes each pipe name unique.
static PIPE_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Creates the output pipe of a pseudoconsole (Q-31): this process's end is an overlapped
/// named-pipe server for the single I/O thread; the console's end is a synchronous client file.
/// The pipe allows only the current user (an owner-only access list, so no other account can
/// open it, even between its creation and the opening of the console's end), has one instance
/// (no second client can connect), refuses remote clients, and must be the first instance of
/// its name (nobody can have created it in advance).
pub(super) fn output_pipe() -> io::Result<(mio::windows::NamedPipe, std::fs::File)> {
    use std::os::windows::io::FromRawHandle as _;
    use windows_sys::Win32::Foundation::GENERIC_WRITE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, OPEN_EXISTING,
        PIPE_ACCESS_INBOUND,
    };
    use windows_sys::Win32::System::Pipes::{
        CreateNamedPipeW, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    };
    let serial = PIPE_SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = format!(r"\\.\pipe\ferrum-pty-{}-{serial}", std::process::id());
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let owner_only = security::OwnerOnly::new()?;
    let attributes = owner_only.attributes();
    // SAFETY: `name` is NUL-terminated and outlives the call; `attributes` points to the
    // owner-only descriptor, alive until the end of this function; the handle is checked below.
    let server = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            0,
            PIPE_BUFFER_BYTES,
            0,
            &raw const attributes,
        )
    };
    if server == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `server` is a valid named-pipe handle that nothing else owns; mio takes ownership
    // and closes it on drop.
    let server = unsafe { mio::windows::NamedPipe::from_raw_handle(server) };
    // SAFETY: `name` is NUL-terminated; no template, no security attributes (not inheritable);
    // the returned handle is checked below.
    let client = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_WRITE,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if client == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `client` is a valid file handle that nothing else owns; `File` closes it on drop.
    Ok((server, unsafe { std::fs::File::from_raw_handle(client) }))
}
