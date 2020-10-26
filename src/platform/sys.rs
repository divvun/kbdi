#![allow(non_snake_case)]

macro_rules! lib_extern {
    ( $($name:ident ( $($arg: ident : $argty: ty),* ) -> $retty: ty);+ ) => {
        $(pub unsafe fn $name($($arg: $argty),*) -> $retty {
            let func: Symbol<unsafe extern "stdcall" fn($($arg: $argty),*) -> $retty> =
                LIB.get(stringify!($name).as_bytes()).unwrap();
            func($($arg),*)
        })+
    };
}

#[cfg(not(feature = "legacy"))]
pub mod bcp47langs {
    use lazy_static::lazy_static;
    use libloading::os::windows::*;
    use winapi::winrt::hstring::HSTRING;
    use winapi::{
        ctypes::*,
        um::winnt::{CHAR, WCHAR},
    };

    lazy_static! {
        static ref LIB: Library = Library::new(r"C:\Windows\System32\BCP47Langs.dll").unwrap();
    }

    lib_extern! {
        GetUserLanguages(delimiter: WCHAR, string: *mut HSTRING) -> c_int;
        GetUserLanguageInputMethods(language: *const WCHAR, delimiter: WCHAR, string: *mut HSTRING) -> c_int;
        LcidFromBcp47(tag: HSTRING, lcid: *mut c_int) -> c_int;
        RemoveInputsForAllLanguagesInternal() -> c_int;
        Bcp47GetIsoLanguageCode(languageTag: HSTRING, isoLanguageCode: *mut HSTRING) -> c_int
    }
}

pub mod input {
    use lazy_static::lazy_static;
    use libloading::os::windows::*;
    use winapi::ctypes::*;
    use winapi::um::winnt::WCHAR;

    lazy_static! {
        static ref LIB: Library = Library::new(r"C:\Windows\System32\input.dll").unwrap();
    }

    lib_extern! {
        InstallLayoutOrTip(tip_string: *const WCHAR, flags: c_int) -> c_int;
        InstallLayoutOrTipUserReg(user_reg: *const WCHAR, system_reg: *const WCHAR, software_reg: *const WCHAR,
            tip_string: *const WCHAR, flags: c_int) -> c_int
    }
}

#[cfg(not(feature = "legacy"))]
pub mod winlangdb {
    use lazy_static::lazy_static;
    use libloading::os::windows::*;
    use winapi::winrt::hstring::HSTRING;
    use winapi::{
        ctypes::*,
        um::winnt::{CHAR, WCHAR},
    };

    lazy_static! {
        static ref LIB: Library = Library::new(r"C:\Windows\System32\winlangdb.dll").unwrap();
    }

    lib_extern! {
        EnsureLanguageProfileExists() -> c_int;
        GetLanguageNames(language: *const WCHAR, autonym: *mut WCHAR, english_name: *mut WCHAR, local_name: *mut WCHAR, script_name: *mut WCHAR) -> c_int;
        SetUserLanguages(delimiter: WCHAR, user_languages: HSTRING) -> c_int;
        GetDefaultInputMethodForLanguage(language: HSTRING, tip_string: *mut HSTRING) -> c_int;
        TransformInputMethodsForLanguage(tip_string: HSTRING, tag: HSTRING, transformed_tip_string: *mut HSTRING) -> c_int
    }
}
