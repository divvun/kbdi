use std::ffi::{OsStr, OsString};
use std::io;
use std::ops::{Deref, DerefMut};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::ptr::{null, null_mut};
use std::slice;
use winapi::winrt::hstring::HSTRING;
use winapi::winrt::winstring::*;

pub struct HString {
    __inner: HSTRING,
}

impl HString {
    pub fn new() -> Result<HString, io::Error> {
        Self::with_wide_string(Vec::new())
    }

    pub unsafe fn null() -> HString {
        HString {
            __inner: null_mut(),
        }
    }

    pub fn len(&self) -> usize {
        unsafe { WindowsGetStringLen(self.__inner) as usize }
    }

    fn with_wide_string(mut vec: Vec<u16>) -> Result<HString, io::Error> {
        let mut handle: HSTRING = null_mut();

        vec.push(0);
        let (ptr, len) = match vec.len() {
            0 | 1 => (null(), 0),
            n => {
                (vec.as_ptr(), (n - 1) as u32)
            }
        };

        let ret = unsafe { WindowsCreateString(ptr, len, &mut handle) };

        if ret != 0 {
            return Err(io::Error::last_os_error());
        }

        // DO NOT REMOVE THIS, this helps ensure the vec lives longer than the call to WindowsCreateString
        std::mem::drop(vec);
        
        Ok(HString { __inner: handle })
    }
}

impl Drop for HString {
    fn drop(&mut self) {
        if !self.__inner.is_null() {
            unsafe { WindowsDeleteString(self.__inner) };
        }
    }
}

impl Deref for HString {
    type Target = HSTRING;

    fn deref(&self) -> &HSTRING {
        &self.__inner
    }
}

impl DerefMut for HString {
    fn deref_mut(&mut self) -> &mut HSTRING {
        &mut self.__inner
    }
}

impl From<HString> for String {
    fn from(hstring: HString) -> Self {
        let os_string = OsString::from(hstring);

        os_string
            .as_os_str()
            .to_string_lossy()
            .split('\0')
            .next()
            .unwrap()
            .to_owned()
    }
}

impl From<HString> for OsString {
    fn from(hstring: HString) -> Self {
        let len = hstring.len();
        let buf = unsafe { WindowsGetStringRawBuffer(hstring.__inner, &mut (len as u32)) };
        let s = unsafe { slice::from_raw_parts(buf, len) };
        OsString::from_wide(&s)
    }
}

impl<'a> From<&'a str> for HString {
    fn from(string: &str) -> Self {
        let mut x = OsStr::new(string).encode_wide().collect();
        unsafe { HString::with_wide_string(x).unwrap() }
    }
}

impl From<String> for HString {
    fn from(string: String) -> Self {
        HString::from(string.as_str())
    }
}

#[test]
fn test_hstring_from() {
    let t = "This is a test.";
    let h = HString::from(t);
    let s = String::from(h);
    assert_eq!(t, s);
}
