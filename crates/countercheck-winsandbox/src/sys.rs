//! The Win32 calls, each wrapped so callers can't misuse them: handles and memory Windows hands
//! out are owned by guards that free them, every pointer we pass points at memory that outlives
//! the call, and every failure is an `io::Error` with Windows' own error code.

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, HANDLE, HANDLE_FLAG_INHERIT, HLOCAL, INVALID_HANDLE_VALUE, LocalFree,
    SetHandleInformation, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, ConvertStringSidToSidW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    ACL, CreateRestrictedToken, DACL_SECURITY_INFORMATION, DISABLE_MAX_PRIVILEGE, GetFileSecurityW,
    GetSecurityDescriptorDacl, GetTokenInformation, PSECURITY_DESCRIPTOR, PSID, SID_AND_ATTRIBUTES,
    SetFileSecurityW, SetTokenInformation, TOKEN_ADJUST_DEFAULT, TOKEN_ASSIGN_PRIMARY,
    TOKEN_DEFAULT_DACL, TOKEN_DUPLICATE, TOKEN_GROUPS, TOKEN_QUERY, TOKEN_USER, TokenDefaultDacl,
    TokenGroups, TokenUser, WRITE_RESTRICTED,
};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::Threading::{
    CREATE_SUSPENDED, CreateProcessAsUserW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess, GetExitCodeProcess,
    InitializeProcThreadAttributeList, OpenProcessToken, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
    PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject,
};

/// `SE_GROUP_LOGON_ID`: the group attribute that marks the logon session's SID.
const SE_GROUP_LOGON_ID: u32 = 0xC000_0000;

/// A NUL-terminated UTF-16 copy of `s`; an embedded NUL is refused (it would cut the string).
fn wide(s: &OsStr) -> io::Result<Vec<u16>> {
    let mut v: Vec<u16> = s.encode_wide().collect();
    if v.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a path or command contains a NUL character",
        ));
    }
    v.push(0);
    Ok(v)
}

/// A handle we own; closed once.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: we own this handle (it came from a successful Win32 call and nobody else
            // closes it), and it is closed exactly once, here.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Memory Windows allocated with `LocalAlloc` for us (a SID, a security descriptor, a string).
struct Local(*mut core::ffi::c_void);

impl Drop for Local {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the pointer came from a Win32 call documented to allocate with LocalAlloc
            // and hand ownership to the caller; it is freed exactly once, here.
            unsafe { LocalFree(self.0 as HLOCAL) };
        }
    }
}

/// Read a NUL-terminated UTF-16 string Windows wrote at `p`.
///
/// # Safety
/// `p` must point at a valid NUL-terminated UTF-16 string that stays alive during the call.
unsafe fn from_wide_nul(p: *const u16) -> String {
    let mut len = 0usize;
    loop {
        let at = p.wrapping_add(len);
        // SAFETY: the caller guarantees a NUL terminator; `at` is at or before it (we stop at
        // the first NUL), so it is in bounds and initialized.
        if unsafe { *at } == 0 {
            break;
        }
        len += 1;
    }
    // SAFETY: `len` elements starting at `p` were just read one by one, so they are valid.
    let slice = unsafe { std::slice::from_raw_parts(p, len) };
    String::from_utf16_lossy(slice)
}

/// A SID in string form (`S-1-5-...`), converted to Windows' binary form.
fn sid_from_str(s: &str) -> io::Result<Local> {
    let w = wide(OsStr::new(s))?;
    let mut sid: PSID = std::ptr::null_mut();
    // SAFETY: `w` is NUL-terminated and alive for the call; `sid` is a valid out pointer.
    let ok = unsafe { ConvertStringSidToSidW(w.as_ptr(), &mut sid) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Local(sid))
}

/// The string form of a binary SID.
///
/// # Safety
/// `sid` must point at a valid SID that stays alive during the call.
unsafe fn sid_to_string(sid: PSID) -> io::Result<String> {
    let mut out: *mut u16 = std::ptr::null_mut();
    // SAFETY: the caller guarantees `sid` is valid; `out` is a valid out pointer.
    let ok = unsafe { ConvertSidToStringSidW(sid, &mut out) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let guard = Local(out.cast());
    // SAFETY: on success Windows wrote a NUL-terminated string at `out`, owned by `guard`.
    let s = unsafe { from_wide_nul(out) };
    drop(guard);
    Ok(s)
}

/// This process's token, opened with the rights needed to read it and derive a restricted one.
fn own_token() -> io::Result<Handle> {
    let mut token: HANDLE = std::ptr::null_mut();
    let access = TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT;
    // SAFETY: GetCurrentProcess takes no arguments and returns a pseudo handle that needs no
    // closing.
    let me = unsafe { GetCurrentProcess() };
    // SAFETY: `me` is the current-process pseudo handle; `token` is a valid out pointer.
    let ok = unsafe { OpenProcessToken(me, access, &mut token) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Handle(token))
}

/// One piece of token information, in a buffer aligned for the structures Windows writes.
fn token_info(token: &Handle, class: i32) -> io::Result<Vec<u64>> {
    let mut needed = 0u32;
    // SAFETY: a size query: a null buffer of length 0, and a valid out pointer for the size.
    unsafe { GetTokenInformation(token.0, class, std::ptr::null_mut(), 0, &mut needed) };
    if needed == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
    let len = u32::try_from(buf.len() * 8).unwrap_or(u32::MAX);
    // SAFETY: `buf` is writable for `len` bytes, 8-byte aligned (enough for every TOKEN_*
    // structure), and alive for the call.
    let ok =
        unsafe { GetTokenInformation(token.0, class, buf.as_mut_ptr().cast(), len, &mut needed) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(buf)
}

/// This process's user SID and logon-session SID, as strings.
pub fn current_sids() -> io::Result<(String, String)> {
    let token = own_token()?;
    let user_buf = token_info(&token, TokenUser)?;
    let user_ptr = user_buf.as_ptr().cast::<TOKEN_USER>();
    // SAFETY: GetTokenInformation(TokenUser) filled `user_buf` (8-byte aligned, alive here) with
    // a TOKEN_USER, so reading its field is in bounds.
    let user_sid = unsafe { (*user_ptr).User.Sid };
    // SAFETY: the SID points inside `user_buf`, which is alive for the call.
    let user = unsafe { sid_to_string(user_sid) }?;
    let groups_buf = token_info(&token, TokenGroups)?;
    let groups = groups_buf.as_ptr().cast::<TOKEN_GROUPS>();
    // SAFETY: GetTokenInformation(TokenGroups) filled the buffer with a TOKEN_GROUPS.
    let count = unsafe { (*groups).GroupCount } as usize;
    // SAFETY: `Groups` is a variable-length array of `count` entries inside the buffer.
    let first = unsafe { std::ptr::addr_of!((*groups).Groups) }.cast::<SID_AND_ATTRIBUTES>();
    for i in 0..count {
        let entry = first.wrapping_add(i);
        // SAFETY: `i < count`, so `entry` is inside the array Windows wrote in `groups_buf`,
        // which is aligned and alive for the borrow.
        let g = unsafe { &*entry };
        if g.Attributes & SE_GROUP_LOGON_ID == SE_GROUP_LOGON_ID {
            // SAFETY: the SID points inside `groups_buf`, which is alive here.
            let logon = unsafe { sid_to_string(g.Sid) }?;
            return Ok((user, logon));
        }
    }
    Err(io::Error::other("this process has no logon session SID"))
}

/// The permissions (DACL) of one file or folder, in SDDL text form. Follows the path like any
/// open: callers must not pass a symlink or junction they don't mean to follow.
pub fn file_dacl(path: &Path) -> io::Result<String> {
    let w = wide(path.as_os_str())?;
    let mut needed = 0u32;
    // SAFETY: a size query: null buffer, length 0, valid out pointer; `w` is NUL-terminated.
    unsafe {
        GetFileSecurityW(
            w.as_ptr(),
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    if needed == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
    let len = u32::try_from(buf.len() * 8).unwrap_or(u32::MAX);
    // SAFETY: `buf` is writable for `len` bytes and alive for the call.
    let ok = unsafe {
        GetFileSecurityW(
            w.as_ptr(),
            DACL_SECURITY_INFORMATION,
            buf.as_mut_ptr().cast(),
            len,
            &mut needed,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut text: *mut u16 = std::ptr::null_mut();
    // SAFETY: `buf` holds the self-relative security descriptor Windows just wrote; `text` is a
    // valid out pointer; the length out pointer may be null.
    let ok = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            buf.as_mut_ptr().cast(),
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let guard = Local(text.cast());
    // SAFETY: on success `text` is a NUL-terminated string owned by `guard`.
    let s = unsafe { from_wide_nul(text) };
    drop(guard);
    Ok(s)
}

/// A security descriptor parsed from SDDL text, owned.
fn descriptor(sddl: &str) -> io::Result<Local> {
    let w = wide(OsStr::new(sddl))?;
    let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: `w` is NUL-terminated and alive; `sd` is a valid out pointer; the size out
    // pointer may be null.
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w.as_ptr(),
            SDDL_REVISION_1,
            &mut sd,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Local(sd))
}

/// Replace the permissions (DACL) of one file or folder with `sddl`. Changes that one object
/// only: nothing is pushed down to what is inside a folder.
pub fn set_file_dacl(path: &Path, sddl: &str) -> io::Result<()> {
    let w = wide(path.as_os_str())?;
    let sd = descriptor(sddl)?;
    // SAFETY: `w` is NUL-terminated; `sd.0` is the valid security descriptor parsed above, alive
    // until `sd` drops after the call.
    let ok = unsafe { SetFileSecurityW(w.as_ptr(), DACL_SECURITY_INFORMATION, sd.0) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// How to start a process with a write-restricted token.
pub struct Restricted<'a> {
    /// The whole command line, already quoted for the program.
    pub command_line: &'a str,
    pub cwd: &'a Path,
    /// A write must also be allowed for one of these SIDs (string form).
    pub restricting_sids: &'a [&'a str],
    /// The token's default permissions for what its processes create (SDDL `D:` text).
    pub default_dacl: &'a str,
}

/// A process started by [`spawn_restricted`]: suspended until [`Child::resume`].
pub struct Child {
    process: Handle,
    thread: Handle,
    pid: u32,
}

impl Child {
    pub fn id(&self) -> u32 {
        self.pid
    }

    /// The process handle's value, for putting the process in a Job Object. Still owned here.
    pub fn raw_handle(&self) -> isize {
        self.process.0 as isize
    }

    /// Let the process run.
    pub fn resume(&self) -> io::Result<()> {
        // SAFETY: `thread` is the process's main thread handle, owned by this Child.
        if unsafe { ResumeThread(self.thread.0) } == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Wait up to `timeout` for the process to end: its exit code, or `None` if it still runs.
    pub fn wait(&self, timeout: Duration) -> io::Result<Option<u32>> {
        let ms = u32::try_from(timeout.as_millis())
            .unwrap_or(u32::MAX - 1)
            .min(u32::MAX - 1);
        // SAFETY: `process` is a process handle owned by this Child.
        match unsafe { WaitForSingleObject(self.process.0, ms) } {
            WAIT_OBJECT_0 => {
                let mut code = 0u32;
                // SAFETY: `process` is valid; `code` is a valid out pointer.
                if unsafe { GetExitCodeProcess(self.process.0, &mut code) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(Some(code))
            }
            WAIT_TIMEOUT => Ok(None),
            _ => Err(io::Error::last_os_error()),
        }
    }

    /// End the process (not what it started: that is the Job Object's work).
    pub fn kill(&self) -> io::Result<()> {
        // SAFETY: `process` is a process handle owned by this Child.
        if unsafe { TerminateProcess(self.process.0, 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

/// An initialized PROC_THREAD_ATTRIBUTE_LIST in an owned, aligned buffer.
struct AttrList(Vec<u64>);

impl AttrList {
    fn ptr(&mut self) -> *mut core::ffi::c_void {
        self.0.as_mut_ptr().cast()
    }
}

impl Drop for AttrList {
    fn drop(&mut self) {
        // SAFETY: the list was initialized by InitializeProcThreadAttributeList (only then is an
        // AttrList built) and is deleted once, here.
        unsafe { DeleteProcThreadAttributeList(self.ptr()) };
    }
}

/// An attribute list that lets the child inherit exactly `handles` (none of the parent's other
/// inheritable handles). `handles` must outlive the list's use in CreateProcess.
fn handle_list(handles: &[HANDLE]) -> io::Result<AttrList> {
    let mut size = 0usize;
    // SAFETY: a size query with a null list, as documented; `size` is a valid out pointer.
    unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size) };
    if size == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buf = vec![0u64; size.div_ceil(8)];
    // SAFETY: `buf` is at least `size` bytes, aligned, and alive for the call.
    if unsafe { InitializeProcThreadAttributeList(buf.as_mut_ptr().cast(), 1, 0, &mut size) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut list = AttrList(buf);
    let ptr = list.ptr();
    // SAFETY: `ptr` is the initialized list; the value points at `handles.len()` HANDLEs that
    // the caller keeps alive until CreateProcess has returned.
    let ok = unsafe {
        UpdateProcThreadAttribute(
            ptr,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            handles.as_ptr().cast(),
            std::mem::size_of_val(handles),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(list)
}

/// This process's stdin, stdout and stderr, made inheritable, without duplicates or empties.
fn std_handles() -> io::Result<[HANDLE; 3]> {
    let mut out = [std::ptr::null_mut(); 3];
    for (slot, which) in out
        .iter_mut()
        .zip([STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE])
    {
        // SAFETY: GetStdHandle only reads the process's standard handle table.
        let h = unsafe { GetStdHandle(which) };
        if h.is_null() || h == INVALID_HANDLE_VALUE {
            continue;
        }
        // SAFETY: `h` is one of this process's open standard handles.
        if unsafe { SetHandleInformation(h, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) } == 0 {
            return Err(io::Error::last_os_error());
        }
        *slot = h;
    }
    Ok(out)
}

/// Start `spec.command_line` suspended, with this process's token restricted for writes to
/// `spec.restricting_sids`, all privileges but traverse removed, and only stdin/stdout/stderr
/// inherited. The caller puts it in a Job Object, then calls [`Child::resume`].
pub fn spawn_restricted(spec: &Restricted<'_>) -> io::Result<Child> {
    let token = own_token()?;
    let sids = spec
        .restricting_sids
        .iter()
        .map(|s| sid_from_str(s))
        .collect::<io::Result<Vec<Local>>>()?;
    let entries: Vec<SID_AND_ATTRIBUTES> = sids
        .iter()
        .map(|s| SID_AND_ATTRIBUTES {
            Sid: s.0,
            Attributes: 0,
        })
        .collect();
    let count = u32::try_from(entries.len()).unwrap_or(0);
    let mut restricted: HANDLE = std::ptr::null_mut();
    // SAFETY: `token` is open with TOKEN_DUPLICATE; no SIDs to disable or privileges to delete
    // (null with count 0); `entries` holds `count` valid SIDs owned by `sids`, alive here.
    let ok = unsafe {
        CreateRestrictedToken(
            token.0,
            DISABLE_MAX_PRIVILEGE | WRITE_RESTRICTED,
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            count,
            entries.as_ptr(),
            &mut restricted,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let restricted = Handle(restricted);

    let sd = descriptor(spec.default_dacl)?;
    let mut present = 0;
    let mut defaulted = 0;
    let mut dacl: *mut ACL = std::ptr::null_mut();
    // SAFETY: `sd.0` is a valid security descriptor; the out pointers are valid. The returned
    // ACL points inside `sd`, which stays alive until the end of this function.
    if unsafe { GetSecurityDescriptorDacl(sd.0, &mut present, &mut dacl, &mut defaulted) } == 0
        || present == 0
        || dacl.is_null()
    {
        return Err(io::Error::other("the default permissions have no DACL"));
    }
    let info = TOKEN_DEFAULT_DACL { DefaultDacl: dacl };
    // SAFETY: `restricted` is a token handle with TOKEN_ADJUST_DEFAULT (inherited from `token`);
    // `info` points at a valid ACL inside `sd`, alive for the call.
    let ok = unsafe {
        SetTokenInformation(
            restricted.0,
            TokenDefaultDacl,
            std::ptr::addr_of!(info).cast(),
            u32::try_from(std::mem::size_of::<TOKEN_DEFAULT_DACL>()).unwrap_or(0),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }

    let std = std_handles()?;
    let mut inherit: Vec<HANDLE> = Vec::new();
    for h in std {
        if !h.is_null() && !inherit.contains(&h) {
            inherit.push(h);
        }
    }
    let mut list = if inherit.is_empty() {
        None
    } else {
        Some(handle_list(&inherit)?)
    };
    let mut si = STARTUPINFOEXW::default();
    si.StartupInfo.cb = u32::try_from(std::mem::size_of::<STARTUPINFOEXW>()).unwrap_or(0);
    si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    si.StartupInfo.hStdInput = std[0];
    si.StartupInfo.hStdOutput = std[1];
    si.StartupInfo.hStdError = std[2];
    si.lpAttributeList = list.as_mut().map_or(std::ptr::null_mut(), AttrList::ptr);

    let mut cmd = wide(OsStr::new(spec.command_line))?;
    let cwd = wide(spec.cwd.as_os_str())?;
    let mut pi = PROCESS_INFORMATION::default();
    // SAFETY: `restricted` is a primary token derived from our own (so no extra privilege is
    // needed); `cmd` is a mutable NUL-terminated buffer, as CreateProcessAsUserW requires;
    // `cwd` is NUL-terminated; `si` is a STARTUPINFOEXW whose size is in `cb` and whose
    // attribute list and handles stay alive until the call returns; `pi` is a valid out struct.
    let ok = unsafe {
        CreateProcessAsUserW(
            restricted.0,
            std::ptr::null(),
            cmd.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            i32::from(list.is_some()),
            CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT,
            std::ptr::null(),
            cwd.as_ptr(),
            std::ptr::addr_of!(si.StartupInfo),
            &mut pi,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    drop(list);
    Ok(Child {
        process: Handle(pi.hProcess),
        thread: Handle(pi.hThread),
        pid: pi.dwProcessId,
    })
}
