use std::ffi::{OsString, OsStr};
use std::iter::once;
use std::os::windows::ffi::{OsStrExt, OsStringExt};

pub mod hstring;
pub use hstring::*;

pub fn to_wide_string(input: &str) -> Vec<u16> {
    OsStr::new(input)
        .encode_wide()
        .chain(once(0))
        .collect()
}

pub fn from_wide_string(vec: &[u16]) -> Result<String, OsString> {
    let s = OsString::from_wide(&vec)
        .into_string()?;
    
    Ok(s.split('\0').next().unwrap().to_owned())
}
