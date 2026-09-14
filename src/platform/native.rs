//! Owned public Windows SDK operations used by the keyboard helper.
use std::{ffi::OsString, io, mem::size_of, os::windows::ffi::OsStringExt, path::PathBuf, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Security::{
        GetTokenInformation, IsWellKnownSid, TOKEN_QUERY, TOKEN_USER, TokenUser, WinLocalSystemSid,
    },
    System::{
        Com::CoTaskMemFree,
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
    UI::Shell::{
        FOLDERID_LocalAppData, FOLDERID_ProgramData, KF_FLAG_DONT_VERIFY, SHGetKnownFolderPath,
    },
};

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this wrapper exclusively owns a successfully opened handle.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub fn hresult(value: i32) -> io::Result<()> {
    if value < 0 {
        Err(io::Error::other(windows_core::Error::from_hresult(
            windows_core::HRESULT(value),
        )))
    } else {
        Ok(())
    }
}

pub fn is_local_system() -> io::Result<bool> {
    let mut token = ptr::null_mut();
    // SAFETY: borrowed process pseudo-handle and writable owned output handle.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = OwnedHandle(token);
    let mut bytes = 0;
    // SAFETY: sizing query writes only the required length.
    unsafe {
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut bytes);
    }
    if bytes < size_of::<TOKEN_USER>() as u32 {
        return Err(io::Error::last_os_error());
    }
    // Pointer alignment accommodates TOKEN_USER and the embedded SID.
    let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
    // SAFETY: aligned owned allocation has at least the declared byte capacity.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            bytes,
            &mut bytes,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful TokenUser query initialized this structure and SID;
    // backing buffer remains alive through IsWellKnownSid.
    Ok(unsafe {
        IsWellKnownSid(
            (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid,
            WinLocalSystemSid,
        )
    } != 0)
}

pub fn log_directory() -> io::Result<PathBuf> {
    let id = if is_local_system()? {
        FOLDERID_ProgramData
    } else {
        FOLDERID_LocalAppData
    };
    let mut path = ptr::null_mut();
    // SAFETY: valid GUID and writable output. DONT_VERIFY does not create paths.
    let result = unsafe {
        SHGetKnownFolderPath(&id, KF_FLAG_DONT_VERIFY as u32, ptr::null_mut(), &mut path)
    };
    hresult(result)?;
    if path.is_null() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    struct OwnedPath(*mut u16);
    impl Drop for OwnedPath {
        fn drop(&mut self) {
            // SAFETY: SHGetKnownFolderPath transfers a CoTaskMem allocation.
            unsafe {
                CoTaskMemFree(self.0.cast());
            }
        }
    }
    let path = OwnedPath(path);
    let mut length = 0;
    // SAFETY: successful Windows result is a NUL-terminated allocated path.
    while unsafe { *path.0.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: the live owned result contains length code units before its NUL.
    let root = OsString::from_wide(unsafe { std::slice::from_raw_parts(path.0, length) });
    Ok(PathBuf::from(root).join("kbdi").join("log"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_and_known_folder_queries_are_read_only() {
        is_local_system().unwrap();
        let path = log_directory().unwrap();
        assert!(path.is_absolute());
        assert!(path.ends_with("kbdi/log"));
    }
    #[test]
    fn hresult_errors_preserve_failure() {
        assert!(hresult(0).is_ok());
        assert!(hresult(1).is_ok());
        assert!(hresult(0x80070005u32 as i32).is_err());
    }
}
