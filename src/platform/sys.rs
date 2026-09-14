//! Private declarations for dynamically available language/input APIs.
//! These are not SDK-supported contracts; keep capability failures explicit.
#![allow(non_snake_case)]
use libloading::os::windows::{LOAD_LIBRARY_SEARCH_SYSTEM32, Library, Symbol};
use std::{io, sync::LazyLock};
#[cfg(not(feature = "legacy"))]
use windows_core::{HSTRING, InRef, OutRef};

fn library(name: &str) -> Result<Library, libloading::Error> {
    // SAFETY: only explicitly named Windows system libraries are loaded, with
    // SYSTEM32 search semantics. Owned static handles outlive all function calls.
    unsafe { Library::load_with_flags(name, LOAD_LIBRARY_SEARCH_SYSTEM32) }
}

macro_rules! lib_extern {
    ($($name:ident($($arg:ident: $argty:ty),*) -> $retty:ty);+ $(;)?) => {
        $(pub unsafe fn $name($($arg: $argty),*) -> io::Result<$retty> {
            let library = LIB.as_ref().map_err(|error| io::Error::new(io::ErrorKind::Unsupported, error.to_string()))?;
            // SAFETY: each declaration is the isolated historical Windows ABI;
            // callers supply valid parameters. Missing exports fail explicitly.
            let function: Symbol<unsafe extern "system" fn($($argty),*) -> $retty> = unsafe {
                library.get(concat!(stringify!($name), "\0").as_bytes())
            }.map_err(|error| io::Error::new(io::ErrorKind::Unsupported, error.to_string()))?;
            // SAFETY: forwarded per-function caller contract; library stays loaded.
            Ok(unsafe { function($($arg),*) })
        })+
    };
}

#[cfg(not(feature = "legacy"))]
pub mod bcp47langs {
    use super::*;
    static LIB: LazyLock<Result<Library, libloading::Error>> =
        LazyLock::new(|| library("BCP47Langs.dll"));
    lib_extern! {
        GetUserLanguages(delimiter: u16, string: OutRef<'_, HSTRING>) -> i32;
        GetUserLanguageInputMethods(language: *const u16, delimiter: u16, string: OutRef<'_, HSTRING>) -> i32;
        LcidFromBcp47(tag: InRef<'_, HSTRING>, lcid: *mut i32) -> i32;
        RemoveInputsForAllLanguagesInternal() -> i32;
        Bcp47GetIsoLanguageCode(languageTag: InRef<'_, HSTRING>, isoLanguageCode: OutRef<'_, HSTRING>) -> i32
    }
}

#[cfg(not(feature = "legacy"))]
pub mod coreglobconfig {
    use super::*;
    static LIB: LazyLock<Result<Library, libloading::Error>> =
        LazyLock::new(|| library("coreglobconfig.dll"));
    lib_extern! { SyncLanguageDataToCloud() -> () }
}

pub mod input {
    use super::*;
    static LIB: LazyLock<Result<Library, libloading::Error>> =
        LazyLock::new(|| library("input.dll"));
    lib_extern! {
        InstallLayoutOrTip(tip_string: *const u16, flags: i32) -> i32;
        InstallLayoutOrTipUserReg(user_reg: *const u16, system_reg: *const u16, software_reg: *const u16, tip_string: *const u16, flags: i32) -> i32
    }
}

#[cfg(not(feature = "legacy"))]
pub mod winlangdb {
    use super::*;
    static LIB: LazyLock<Result<Library, libloading::Error>> =
        LazyLock::new(|| library("winlangdb.dll"));
    lib_extern! {
        EnsureLanguageProfileExists() -> i32;
        GetLanguageNames(language: *const u16, autonym: *mut u16, english_name: *mut u16, local_name: *mut u16, script_name: *mut u16) -> i32;
        SetUserLanguages(delimiter: u16, user_languages: InRef<'_, HSTRING>) -> i32;
        GetDefaultInputMethodForLanguage(language: InRef<'_, HSTRING>, tip_string: OutRef<'_, HSTRING>) -> i32;
        TransformInputMethodsForLanguage(tip_string: InRef<'_, HSTRING>, tag: InRef<'_, HSTRING>, transformed_tip_string: OutRef<'_, HSTRING>) -> i32
    }
}
