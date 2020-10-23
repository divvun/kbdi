use crate::platform::*;
use std::io;
use crate::winrust::*;
use crate::winrust::hstring::*;

pub fn get_user_languages() -> Result<Vec<String>, io::Error> {
    let handle = unsafe {
        let mut hstring = HString::null();
        let ret = sys::GetUserLanguages(';' as u16, &mut *hstring);
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        hstring
    };
        
    let langs = String::from(handle);
    Ok(langs.split(';').map(|x| x.to_owned()).collect())
}

pub fn get_user_language_input_methods(tag: &str) -> Result<Vec<String>, io::Error> {
    let wtag = to_wide_string(tag);

    let handle = unsafe {
        let mut hstring = HString::null();
        let ret = sys::GetUserLanguageInputMethods(wtag.as_ptr(), ';' as u16, &mut *hstring);
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        hstring
    };
        
    let langs = String::from(handle);
    if langs == "" {
        return Ok(vec![]);
    }
    Ok(langs.split(';').map(|x| x.to_owned()).collect())
}

pub fn lcid_from_bcp47(tag: &str) -> Option<u32> {
    let handle = HString::from(tag);
    let mut lcid = 0i32;

    unsafe { sys::LcidFromBcp47(*handle, &mut lcid) };

    match lcid {
        0 => None,
        _ => Some(lcid as u32)
    }
}

pub fn remove_inputs_for_all_languages() -> Result<(), io::Error> {
    let ret = unsafe { sys::RemoveInputsForAllLanguagesInternal() };

    if ret < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

mod sys {
    use winapi::winrt::hstring::HSTRING;
    use winapi::ctypes::{c_char, c_int};
    use winapi::um::winnt::WCHAR;
    use libloading::os::windows::*;
    use lazy_static::lazy_static;

    macro_rules! lib_extern {
        ( $($name:ident ( $($arg: ident : $argty: ty),* ) -> $retty: ty);+ ) => {
            $(pub unsafe fn $name($($arg: $argty),*) -> $retty {
                let func: Symbol<unsafe extern "stdcall" fn($($arg: $argty),*) -> $retty> =
                    LIB.get(stringify!($name).as_bytes()).unwrap();
                func($($arg),*)
            })+
        };
    }

    lazy_static! {
        static ref LIB: Library = Library::new(r"C:\Windows\System32\BCP47Langs.dll").unwrap();
    }

    lib_extern! {
        GetUserLanguages(delimiter: u16, string: *mut HSTRING) -> c_int;
        GetUserLanguageInputMethods(language: *const WCHAR, delimiter: u16, string: *mut HSTRING) -> c_int;
        LcidFromBcp47(tag: HSTRING, lcid: *mut c_int) -> c_int;
        RemoveInputsForAllLanguagesInternal() -> c_int
    }
}
