//! Owned registry snapshots with explicit error handling.
//! windows-registry's convenience iterators stop on native enumeration errors;
//! maintenance code must not mistake a partial scan for a complete snapshot.
use windows_registry::{Key, Value};
use windows_result::{Error, HRESULT, Result};
use windows_sys::Win32::{Foundation::*, System::Registry::*};

fn failure(code: u32) -> Error {
    Error::from_hresult(HRESULT((0x80070000 | code) as i32))
}

#[allow(dead_code)]
pub(crate) fn keys(key: &Key) -> Result<Vec<String>> {
    let mut result = Vec::new();
    let mut index = 0;
    loop {
        let mut name = vec![0; 256];
        loop {
            let mut length = name.len() as u32;
            let status = unsafe {
                RegEnumKeyExW(
                    key.as_raw(),
                    index,
                    name.as_mut_ptr(),
                    &mut length,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            match status {
                ERROR_SUCCESS => {
                    result.push(
                        String::from_utf16(&name[..length as usize])
                            .map_err(|_| failure(ERROR_INVALID_DATA))?,
                    );
                    index += 1;
                    break;
                }
                ERROR_NO_MORE_ITEMS => return Ok(result),
                ERROR_MORE_DATA if name.len() < 32768 => name.resize(name.len() * 2, 0),
                error => return Err(failure(error)),
            }
        }
    }
}

pub(crate) fn values(key: &Key) -> Result<Vec<(String, Value)>> {
    let mut result = Vec::new();
    let mut index = 0;
    loop {
        let mut name = vec![0; 256];
        let mut bytes = vec![0; 256];
        loop {
            let mut name_length = name.len() as u32;
            let mut data_length = bytes.len() as u32;
            let mut kind = 0;
            let status = unsafe {
                RegEnumValueW(
                    key.as_raw(),
                    index,
                    name.as_mut_ptr(),
                    &mut name_length,
                    std::ptr::null(),
                    &mut kind,
                    bytes.as_mut_ptr(),
                    &mut data_length,
                )
            };
            match status {
                ERROR_SUCCESS => {
                    let name = String::from_utf16(&name[..name_length as usize])
                        .map_err(|_| failure(ERROR_INVALID_DATA))?;
                    let mut value = Value::from(&bytes[..data_length as usize]);
                    value.set_ty(kind.into());
                    result.push((name, value));
                    index += 1;
                    break;
                }
                ERROR_NO_MORE_ITEMS => return Ok(result),
                ERROR_MORE_DATA => {
                    if name.len() >= 32768 && bytes.len() >= 64 * 1024 * 1024 {
                        return Err(failure(ERROR_MORE_DATA));
                    }
                    name.resize((name.len() * 2).min(32768), 0);
                    let size = (bytes.len() * 2)
                        .max(data_length as usize)
                        .min(64 * 1024 * 1024);
                    bytes.resize(size, 0);
                }
                error => return Err(failure(error)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_native_registry_without_creating_or_changing_keys() {
        let key = windows_registry::LOCAL_MACHINE
            .open(r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts")
            .unwrap();
        let names = keys(&key).unwrap();
        assert!(!names.is_empty());
        let child = key.open(&names[0]).unwrap();
        assert!(!values(&child).unwrap().is_empty());
    }
}
