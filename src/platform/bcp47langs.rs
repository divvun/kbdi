use crate::platform::*;
use crate::winrust::*;
use std::io;
use windows_core::HSTRING;

pub fn get_user_languages() -> io::Result<Vec<String>> {
    let mut output = HSTRING::new();
    // SAFETY: fresh owned HSTRING output; the system transfers ownership to it.
    let result = unsafe { sys::bcp47langs::GetUserLanguages(';' as u16, (&mut output).into())? };
    super::native::hresult(result)?;
    Ok(output
        .to_string_lossy()
        .split(';')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect())
}

pub fn get_user_language_input_methods(tag: &str) -> io::Result<Vec<String>> {
    let tag = to_wide_string(tag);
    let mut output = HSTRING::new();
    // SAFETY: owned NUL-terminated input and fresh owned HSTRING output.
    let result = unsafe {
        sys::bcp47langs::GetUserLanguageInputMethods(
            tag.as_ptr(),
            ';' as u16,
            (&mut output).into(),
        )?
    };
    super::native::hresult(result)?;
    Ok(output
        .to_string_lossy()
        .split(';')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect())
}

pub fn lcid_from_bcp47(tag: &str) -> Option<u32> {
    let tag = HSTRING::from(tag);
    let mut lcid = 0i32;
    // SAFETY: borrowed HSTRING lives through the call, writable local integer.
    let result = unsafe { sys::bcp47langs::LcidFromBcp47((&tag).into(), &mut lcid).ok()? };
    super::native::hresult(result).ok()?;
    (lcid != 0).then_some(lcid as u32)
}

pub fn bcp47_get_iso_language_code(tag: &str) -> io::Result<String> {
    let tag = HSTRING::from(tag);
    let mut output = HSTRING::new();
    // SAFETY: borrowed input and fresh output have their native HSTRING layout.
    let result =
        unsafe { sys::bcp47langs::Bcp47GetIsoLanguageCode((&tag).into(), (&mut output).into())? };
    super::native::hresult(result)?;
    Ok(output.to_string_lossy())
}

pub fn remove_inputs_for_all_languages() -> io::Result<()> {
    // SAFETY: no pointer arguments; explicit mutating API used by callers only.
    super::native::hresult(unsafe { sys::bcp47langs::RemoveInputsForAllLanguagesInternal()? })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_hstring_output_and_borrowed_input_on_real_windows() {
        let languages = get_user_languages().unwrap();
        assert!(!languages.is_empty());
        assert!(languages.iter().all(|tag| !tag.contains('\0')));
        assert_eq!(lcid_from_bcp47("en-US"), Some(1033));
    }
}
