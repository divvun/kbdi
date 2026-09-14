//! Recovery touches only ctfmon in the calling user's interactive session.
use super::native::{OwnedHandle, is_local_system};
use crate::winrust::to_wide_string;
use std::{
    io,
    mem::{size_of, zeroed},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        ERROR_NO_MORE_FILES, HANDLE, INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0,
    },
    Security::*,
    System::{
        Diagnostics::ToolHelp::*, RemoteDesktop::ProcessIdToSessionId, StationsAndDesktops::*,
        SystemInformation::GetWindowsDirectoryW, Threading::*,
    },
    UI::{Input::KeyboardAndMouse::GetKeyboardLayout, WindowsAndMessaging::*},
};

fn checked(value: i32) -> io::Result<()> {
    if value == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn token(process: HANDLE, rights: u32) -> io::Result<OwnedHandle> {
    let mut value = ptr::null_mut();
    checked(unsafe { OpenProcessToken(process, rights, &mut value) })?;
    Ok(OwnedHandle(value))
}
fn token_data(token: HANDLE, class: TOKEN_INFORMATION_CLASS) -> io::Result<Vec<usize>> {
    let mut length = 0;
    unsafe {
        GetTokenInformation(token, class, ptr::null_mut(), 0, &mut length);
    }
    if length == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut data = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
    checked(unsafe {
        GetTokenInformation(token, class, data.as_mut_ptr().cast(), length, &mut length)
    })?;
    Ok(data)
}
fn user(process: HANDLE) -> io::Result<Vec<u8>> {
    let token = token(process, TOKEN_QUERY)?;
    let data = token_data(token.0, TokenUser)?;
    // SAFETY: aligned TokenUser allocation remains live while copying its SID.
    let sid = unsafe { (*data.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    Ok(
        unsafe { std::slice::from_raw_parts(sid.cast::<u8>(), GetLengthSid(sid) as usize) }
            .to_vec(),
    )
}
fn session(pid: u32) -> io::Result<u32> {
    let mut value = 0;
    checked(unsafe { ProcessIdToSessionId(pid, &mut value) })?;
    Ok(value)
}
fn object_name(object: HANDLE) -> io::Result<String> {
    let mut buffer = [0u16; 256];
    let mut length = 0;
    checked(unsafe {
        GetUserObjectInformationW(
            object,
            UOI_NAME,
            buffer.as_mut_ptr().cast(),
            size_of_val(&buffer) as u32,
            &mut length,
        )
    })?;
    Ok(String::from_utf16_lossy(
        &buffer[..buffer.iter().position(|x| *x == 0).unwrap_or(buffer.len())],
    ))
}
fn image_path(process: HANDLE) -> io::Result<String> {
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    checked(unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) })?;
    Ok(String::from_utf16_lossy(&buffer[..length as usize]))
}

pub struct Session {
    id: u32,
    sid: Vec<u8>,
    windows: String,
    shell: OwnedHandle,
    _lock: SessionLock,
}
struct SessionLock(OwnedHandle);
impl Drop for SessionLock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0.0);
        }
    }
}
impl Session {
    /// Refuse service accounts, alternate desktops and another user's shell.
    /// The mutex serializes cooperating kbdi refreshes in this logon session.
    pub fn current() -> io::Result<Self> {
        let id = session(unsafe { GetCurrentProcessId() })?;
        if id == 0
            || is_local_system()?
            || !object_name(unsafe { GetProcessWindowStation() })?.eq_ignore_ascii_case("WinSta0")
            || !object_name(unsafe { GetThreadDesktop(GetCurrentThreadId()) })?
                .eq_ignore_ascii_case("Default")
        {
            return Err(io::Error::other(
                "keyboard refresh requires the user's interactive desktop; run keyboard_refresh there or sign out and back in",
            ));
        }
        let sid = user(unsafe { GetCurrentProcess() })?;
        let mut shell_pid = 0;
        let shell_window = unsafe { GetShellWindow() };
        if shell_window.is_null() {
            return Err(io::Error::other(
                "no interactive shell for keyboard refresh",
            ));
        }
        unsafe {
            GetWindowThreadProcessId(shell_window, &mut shell_pid);
        }
        if session(shell_pid)? != id {
            return Err(io::Error::other("shell session differs from caller"));
        }
        let shell =
            OwnedHandle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, shell_pid) });
        if shell.0.is_null() {
            return Err(io::Error::last_os_error());
        }
        if user(shell.0)? != sid {
            return Err(io::Error::other("shell user differs from caller"));
        }
        let mut windows = [0u16; 32768];
        let length = unsafe { GetWindowsDirectoryW(windows.as_mut_ptr(), windows.len() as u32) };
        if length == 0 || length as usize >= windows.len() {
            return Err(io::Error::last_os_error());
        }
        let windows = String::from_utf16_lossy(&windows[..length as usize]);
        if !image_path(shell.0)?.eq_ignore_ascii_case(&format!("{windows}\\explorer.exe")) {
            return Err(io::Error::other("unsupported interactive shell"));
        }
        let name = to_wide_string(&format!("Local\\kbdi-text-services-{id}"));
        let lock = OwnedHandle(unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) });
        if lock.0.is_null() {
            return Err(io::Error::last_os_error());
        }
        match unsafe { WaitForSingleObject(lock.0, 15000) } {
            WAIT_OBJECT_0 | WAIT_ABANDONED => (),
            _ => {
                return Err(io::Error::other(
                    "another keyboard refresh is still running",
                ));
            }
        }
        Ok(Self {
            id,
            sid,
            windows,
            shell,
            _lock: SessionLock(lock),
        })
    }

    fn ctfmon(&self, terminate: bool) -> io::Result<Option<OwnedHandle>> {
        let snapshot = OwnedHandle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) });
        if snapshot.0 == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let mut entry: PROCESSENTRY32W = unsafe { zeroed() };
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) };
        let mut found = None;
        while more != 0 {
            let name = String::from_utf16_lossy(
                &entry.szExeFile[..entry
                    .szExeFile
                    .iter()
                    .position(|x| *x == 0)
                    .unwrap_or(entry.szExeFile.len())],
            );
            if name.eq_ignore_ascii_case("ctfmon.exe")
                && session(entry.th32ProcessID).ok() == Some(self.id)
            {
                let rights = PROCESS_QUERY_LIMITED_INFORMATION
                    | PROCESS_SYNCHRONIZE
                    | if terminate { PROCESS_TERMINATE } else { 0 };
                let process = OwnedHandle(unsafe { OpenProcess(rights, 0, entry.th32ProcessID) });
                if process.0.is_null() {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() == Some(87) {
                        // ERROR_INVALID_PARAMETER: process already exited
                        more = unsafe { Process32NextW(snapshot.0, &mut entry) };
                        continue;
                    }
                    return Err(error);
                }
                // Validate the open handle, not just the enumeration's PID/name.
                let token = token(process.0, TOKEN_QUERY)?;
                let token_session = token_data(token.0, TokenSessionId)?;
                if token_session[0] as u32 != self.id
                    || user(process.0)? != self.sid
                    || !image_path(process.0)?
                        .eq_ignore_ascii_case(&format!("{}\\System32\\ctfmon.exe", self.windows))
                {
                    return Err(io::Error::other(
                        "ctfmon identity/path does not match this session",
                    ));
                }
                if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_OBJECT_0 {
                    if found.is_some() {
                        return Err(io::Error::other(
                            "multiple ctfmon processes in this session; refusing ambiguous recovery",
                        ));
                    }
                    found = Some(process);
                }
            }
            more = unsafe { Process32NextW(snapshot.0, &mut entry) };
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
            return Err(error);
        }
        Ok(found)
    }

    pub fn restart(&self) -> io::Result<()> {
        // Prepare a medium-integrity launch before stopping anything. Elevated
        // installers use the verified same-user shell token, never their own.
        let own_token = token(unsafe { GetCurrentProcess() }, TOKEN_QUERY)?;
        let elevated = token_data(own_token.0, TokenElevation)?[0] as u32 != 0;
        let launch_token = if elevated {
            let shell_token = token(self.shell.0, TOKEN_QUERY | TOKEN_DUPLICATE)?;
            let mut duplicate = ptr::null_mut();
            checked(unsafe {
                DuplicateTokenEx(
                    shell_token.0,
                    TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY,
                    ptr::null(),
                    SecurityImpersonation,
                    TokenPrimary,
                    &mut duplicate,
                )
            })?;
            Some(OwnedHandle(duplicate))
        } else {
            None
        };
        if let Some(process) = self.ctfmon(true)? {
            log::warn!(
                "Refreshing stale text services for user session {}",
                self.id
            );
            checked(unsafe { TerminateProcess(process.0, 0) })?;
            if unsafe { WaitForSingleObject(process.0, 3000) } != WAIT_OBJECT_0 {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "ctfmon did not exit",
                ));
            }
        }
        // Windows normally respawns it. Launch only if that did not happen.
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if self.ctfmon(false)?.is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let native_dir = if cfg!(target_arch = "x86")
            && std::path::Path::new(&format!("{}\\Sysnative\\ctfmon.exe", self.windows)).exists()
        {
            "Sysnative"
        } else {
            "System32"
        };
        let executable = to_wide_string(&format!("{}\\{native_dir}\\ctfmon.exe", self.windows));
        let mut desktop = to_wide_string("WinSta0\\Default");
        let mut startup: STARTUPINFOW = unsafe { zeroed() };
        startup.cb = size_of::<STARTUPINFOW>() as u32;
        startup.lpDesktop = desktop.as_mut_ptr();
        startup.dwFlags = STARTF_USESHOWWINDOW;
        startup.wShowWindow = SW_HIDE as u16;
        let mut info: PROCESS_INFORMATION = unsafe { zeroed() };
        let launched = unsafe {
            if let Some(token) = launch_token {
                CreateProcessWithTokenW(
                    token.0,
                    0,
                    executable.as_ptr(),
                    ptr::null_mut(),
                    CREATE_NO_WINDOW,
                    ptr::null(),
                    ptr::null(),
                    &startup,
                    &mut info,
                )
            } else {
                CreateProcessW(
                    executable.as_ptr(),
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    CREATE_NO_WINDOW,
                    ptr::null(),
                    ptr::null(),
                    &startup,
                    &mut info,
                )
            }
        };
        checked(launched)?;
        let _process = OwnedHandle(info.hProcess);
        let _thread = OwnedHandle(info.hThread);
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if self.ctfmon(false)?.is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(io::Error::other(
            "ctfmon did not start; sign out and back in to refresh text services",
        ))
    }
}

/// Restore the foreground application's selection, not the installer's HKL.
pub struct ActiveKeyboard(isize);
impl ActiveKeyboard {
    pub fn capture() -> Self {
        let window = unsafe { GetForegroundWindow() };
        let thread = unsafe { GetWindowThreadProcessId(window, ptr::null_mut()) };
        Self(unsafe { GetKeyboardLayout(thread) } as isize)
    }
}
impl Drop for ActiveKeyboard {
    fn drop(&mut self) {
        if self.0 != 0 {
            super::winuser::set_active_keyboard(self.0);
        }
    }
}
