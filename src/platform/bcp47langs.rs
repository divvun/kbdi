use platform::*;
use std::io;
use winrust::*;
use winrust::hstring::*;

pub fn get_user_languages() -> Result<Vec<String>, io::Error> {
    let handle = unsafe {
        let mut hstring = HString::null();
        let ret = sys::bcp47langs::GetUserLanguages(';' as i8, &mut *hstring);
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
        let ret = sys::bcp47langs::GetUserLanguageInputMethods(wtag.as_ptr(), ';' as i8, &mut *hstring);
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

    unsafe { sys::bcp47langs::LcidFromBcp47(*handle, &mut lcid) };

    match lcid {
        0 => None,
        _ => Some(lcid as u32)
    }
}

pub fn remove_inputs_for_all_languages() -> Result<(), io::Error> {
    let ret = unsafe { sys::bcp47langs::RemoveInputsForAllLanguagesInternal() };

    if ret < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}