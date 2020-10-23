use crate::platform::*;
use std::{convert::TryFrom, io};
use std::fmt;
use crate::types::*;   
use crate::winrust::*;
use crate::winrust::hstring::*;

pub struct LanguageData {
    pub tag: String,
    pub name: String,
    pub english_name: String,
    pub localised_name: String,
    pub script_name: String
}

impl fmt::Display for LanguageData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Tag:           {}", self.tag)?;
        writeln!(f, "Name:          {}", self.name)?;
        writeln!(f, "English Name:  {}", self.english_name)?;
        writeln!(f, "Native Name:   {}", self.localised_name)?;
        writeln!(f, "Script:        {}", self.script_name)?;

        Ok(())
    }
}

// pub fn remove_inputs_for_all_languages_internal() -> Result<(), io::Error> {
//     let ret = unsafe { sys::winlangdb::RemoveInputsForAllLanguagesInternal() };

//     if ret < 0 {
//         return Err(io::Error::last_os_error());
//     }

//     Ok(())
// }

pub fn ensure_language_profile_exists() -> Result<(), io::Error> {
    let ret = unsafe { sys::EnsureLanguageProfileExists() };

    if ret < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

pub fn get_language_names(tag: &str) -> Option<LanguageData> {
    let mut a = [0u16; 256];
    let mut b = [0u16; 256];
    let mut c = [0u16; 256];
    let mut d = [0u16; 256];

    let ret = unsafe {
        sys::GetLanguageNames(
            to_wide_string(tag).as_ptr(),
            a.as_mut_ptr(),
            b.as_mut_ptr(),
            c.as_mut_ptr(),
            d.as_mut_ptr()
        )
    };

    if ret < 0 {
        info!("{:?}", io::Error::last_os_error());
        return None;
    }

    Some(LanguageData {
        tag: tag.to_owned(),
        name: from_wide_string(&a).unwrap(),
        english_name: from_wide_string(&b).unwrap(),
        localised_name: from_wide_string(&c).unwrap(),
        script_name: from_wide_string(&d).unwrap()
    })
}

pub fn set_user_languages(tags: &[String]) -> Result<(), io::Error> {
    info!("Set user languages: {:?}", &tags);
    let handle = HString::from(tags.join(";"));
    let ret = unsafe { sys::SetUserLanguages(';' as u16, *handle) };
    
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

pub fn transform_input_methods(methods: InputList, tag: &str) -> InputList {
    let hmethods = HString::from(String::from(methods));
    let htag = HString::from(tag);
    let out = unsafe {
        let mut out = HString::null();
        sys::TransformInputMethodsForLanguage(*hmethods, *htag, &mut *out);
        out
    };
    InputList::try_from(String::from(out)).unwrap()
}

pub fn default_input_method(tag: &str) -> InputList {
    let htag = HString::from(tag);
    let out = unsafe {
        let mut out = HString::null();
        sys::GetDefaultInputMethodForLanguage(*htag, &mut *out);
        out
    };
    InputList::try_from(String::from(out)).unwrap()
}

mod sys {
    use winapi::ctypes::*;
    use winapi::um::winnt::WCHAR;
    use winapi::winrt::hstring::HSTRING;
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
        static ref LIB: Library = Library::new(r"C:\Windows\System32\winlangdb.dll").unwrap();
    }

    lib_extern! {
        EnsureLanguageProfileExists() -> c_int;
        GetLanguageNames(language: *const WCHAR, autonym: *mut WCHAR, english_name: *mut WCHAR, local_name: *mut WCHAR, script_name: *mut WCHAR) -> c_int;
        SetUserLanguages(delimiter: u16, user_languages: HSTRING) -> c_int;
        GetDefaultInputMethodForLanguage(language: HSTRING, tip_string: *mut HSTRING) -> c_int;
        TransformInputMethodsForLanguage(tip_string: HSTRING, tag: HSTRING, transformed_tip_string: *mut HSTRING) -> c_int
    }
}