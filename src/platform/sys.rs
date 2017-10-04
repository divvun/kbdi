#![cfg(windows)]
extern crate winapi;

pub mod bcp47langs {
    use winapi::winrt::hstring::HSTRING;
    use winapi::ctypes::{c_char, c_int};
    
    #[link(name = "BCP47Langs")]
    extern "system" {
        pub fn GetUserLanguages(delimiter: c_char, string: *mut HSTRING) -> c_int;
        pub fn LcidFromBcp47(tag: HSTRING, lcid: *mut c_int) -> c_int;
    }
}

pub mod input {
    use winapi::ctypes::*;
    use winapi::um::winnt::WCHAR;
    
    #[link(name = "input")]
    extern "system" {
        pub fn InstallLayoutOrTip(tip_string: *const WCHAR, flags: c_int) -> c_int;
    }
}

pub mod winlangdb {
    use winapi::ctypes::*;
    use winapi::um::winnt::WCHAR;
    use winapi::winrt::hstring::HSTRING;
    
    #[link(name = "winlangdb")]
    extern "system" {
        pub fn EnsureLanguageProfileExists() -> c_int;
        pub fn GetLanguageNames(language: *const WCHAR, autonym: *mut WCHAR, english_name: *mut WCHAR, local_name: *mut WCHAR, script_name: *mut WCHAR) -> c_int;
        pub fn RemoveInputsForAllLanguagesInternal() -> c_int;
        pub fn SetUserLanguages(delimiter: c_char, user_languages: HSTRING) -> c_int;
        pub fn GetDefaultInputMethodForLanguage(language: HSTRING, tip_string: *mut HSTRING) -> c_int;
        pub fn TransformInputMethodsForLanguage(tip_string: HSTRING, tag: HSTRING, transformed_tip_string: *mut HSTRING) -> c_int;
    }
}
