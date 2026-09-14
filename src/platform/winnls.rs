use crate::winrust::{from_wide_string, to_wide_string};
use std::ffi::c_int;
use std::io;
use windows_sys::Win32::Globalization as sys_winnls;

const MAX_LOCALE_NAME_LEN: usize = 85usize;

pub fn resolve_locale_name(tag: &str) -> Option<String> {
    if tag.is_empty() || tag.contains('\0') {
        return None;
    }
    let mut buf = vec![0u16; MAX_LOCALE_NAME_LEN];

    let ret = unsafe {
        sys_winnls::ResolveLocaleName(
            to_wide_string(tag).as_ptr(),
            buf.as_mut_ptr(),
            MAX_LOCALE_NAME_LEN as c_int,
        )
    };

    if ret == 0 {
        let err = io::Error::last_os_error();
        log::debug!("Cannot resolve locale {tag:?}: {err}");
        return None;
    }

    buf.truncate(ret as usize - 1);

    if buf.len() == 0 {
        return None;
    }

    from_wide_string(&buf).ok()
}

pub fn locale_name_to_lcid(locale_name: &str) -> Result<u32, io::Error> {
    if locale_name.is_empty() || locale_name.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid locale name",
        ));
    }
    let tag = resolve_locale_name(locale_name).unwrap_or(locale_name.to_owned());

    let ret = unsafe { sys_winnls::LocaleNameToLCID(to_wide_string(&tag).as_ptr(), 0) };

    match ret {
        0 => Err(io::Error::last_os_error()),
        _ => Ok(ret),
    }
}
