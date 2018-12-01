use std::io;
use winapi::ctypes::c_int;
use winapi::um::winnls as sys_winnls;
use crate::winrust::{from_wide_string, to_wide_string};

const MAX_LOCALE_NAME_LEN: usize = 85usize;

// TODO: remove panics
pub fn resolve_locale_name(tag: &str) -> Option<String> {
    let mut buf = vec![0u16; MAX_LOCALE_NAME_LEN];

    let ret = unsafe {
        sys_winnls::ResolveLocaleName(
            to_wide_string(tag).as_ptr(),
            buf.as_mut_ptr(),
            MAX_LOCALE_NAME_LEN as c_int
        )
    };
    
    if ret == 0 {
        let err = io::Error::last_os_error();
        info!("{:?}", err);
        panic!();
    }

    buf.truncate(ret as usize - 1);

    if buf.len() == 0 {
        return None;
    }

    Some(from_wide_string(&buf).unwrap())
}

pub fn locale_name_to_lcid(locale_name: &str) -> Result<u32, io::Error> {
    let tag = resolve_locale_name(locale_name)
        .unwrap_or(locale_name.to_owned());

    let ret = unsafe {
        sys_winnls::LocaleNameToLCID(to_wide_string(&tag).as_ptr(), 0)
    };

    match ret {
        0 => Err(io::Error::last_os_error()),
        _ => Ok(ret)
    }
}
