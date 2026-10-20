//! Read the same serialized input profiles consumed by Explorer's switcher.
//!
//! This is a private WinRT contract, isolated here and invoked only in a bounded
//! child process. These IIDs/vtables were checked against Windows 11 26100.9278
//! public symbols. No process memory offsets, DLL patches, or debugger needed.
//! An IID fixes its interface's vtable, so a Windows whose class is missing or
//! answers `E_NOINTERFACE` for one of these IIDs lacks the contract: Windows 10
//! 19044 has the class, but its profiles lack `ICoreKeyboardInputProfile`.
//! `enabled` then fails with `ErrorKind::Unsupported`, which says nothing about
//! the profiles. Every other failure must fail explicitly, never mean
//! "healthy" or "stale".
use super::native::hresult;
use crate::tsf::braced;
use std::{ffi::c_void, io, ptr};
use windows_core::GUID;
use windows_sys::Win32::{
    Foundation::{E_NOINTERFACE, REGDB_E_CLASSNOTREG},
    UI::WindowsAndMessaging::{DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage},
};

const CLASS: &str = "Windows.UI.Internal.Text.Core.CoreKeyboardInputProfileManager";
const IID_STATICS: u128 = 0xd0380bbe_201e_4a70_8eff_bb52f7996cae;
const IID_MANAGER: u128 = 0xb3b6947d_7825_46a6_82a8_496216b9dd28;
const IID_PROFILE: u128 = 0x29fc1d95_e12c_43d9_98a5_de79a87ea36b;

/// Whether `hr` says that Windows has no such class or interface.
fn missing_contract(hr: i32) -> bool {
    hr == E_NOINTERFACE || hr == REGDB_E_CLASSNOTREG
}

/// `hresult`, except that a missing class or interface, `what`, fails with
/// `ErrorKind::Unsupported`.
fn contract(hr: i32, what: impl FnOnce() -> String) -> io::Result<()> {
    if missing_contract(hr) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "{} unavailable: {}",
                what(),
                windows_core::Error::from_hresult(windows_core::HRESULT(hr))
            ),
        ));
    }
    hresult(hr)
}

type Raw = *mut c_void;
type GetObject = unsafe extern "system" fn(Raw, *mut Raw) -> i32;

#[repr(C)]
struct Inspectable {
    query: unsafe extern "system" fn(Raw, *const GUID, *mut Raw) -> i32,
    add_ref: unsafe extern "system" fn(Raw) -> u32,
    release: unsafe extern "system" fn(Raw) -> u32,
    get_iids: usize,
    class_name: usize,
    trust_level: usize,
}
#[repr(C)]
struct Statics {
    base: Inspectable,
    get_instance: GetObject,
}
#[repr(C)]
struct Manager {
    base: Inspectable,
    // Events and mutators (slots 6..18) are deliberately not exposed.
    unused: [usize; 13],
    active: GetObject,
    enabled: GetObject,
}
#[repr(C)]
struct Vector {
    base: Inspectable,
    get_at: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    size: unsafe extern "system" fn(Raw, *mut u32) -> i32,
}
#[repr(C)]
struct Profile {
    base: Inspectable,
    unused: [usize; 18],
    as_tip_string: GetObject,
}

#[link(name = "runtimeobject")]
unsafe extern "system" {
    fn RoInitialize(kind: u32) -> i32;
    fn RoUninitialize();
    fn RoGetActivationFactory(class: Raw, iid: *const GUID, result: *mut Raw) -> i32;
    fn WindowsCreateString(value: *const u16, length: u32, result: *mut Raw) -> i32;
    fn WindowsDeleteString(value: Raw) -> i32;
    fn WindowsGetStringRawBuffer(value: Raw, length: *mut u32) -> *const u16;
}

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() }
    }
}
struct HString(Raw);
impl Drop for HString {
    fn drop(&mut self) {
        unsafe {
            WindowsDeleteString(self.0);
        }
    }
}
struct Com(Raw);
impl Com {
    fn new(raw: Raw) -> io::Result<Self> {
        if raw.is_null() {
            return Err(io::Error::other("null WinRT profile interface"));
        }
        Ok(Self(raw))
    }
    // SAFETY: callers select the vtable corresponding to an explicitly queried
    // IID, or the typed IVectorView returned by ICoreKeyboardInputProfileManager.
    unsafe fn table<T>(&self) -> &T {
        unsafe { &**self.0.cast::<*const T>() }
    }
    fn query(&self, iid: u128, name: &str) -> io::Result<Self> {
        let mut raw = ptr::null_mut();
        let hr =
            unsafe { (self.table::<Inspectable>().query)(self.0, &GUID::from_u128(iid), &mut raw) };
        contract(hr, || format!("{name} {}", braced(iid)))?;
        Self::new(raw)
    }
    fn object(&self, get: GetObject) -> io::Result<Self> {
        let mut raw = ptr::null_mut();
        hresult(unsafe { get(self.0, &mut raw) })?;
        Self::new(raw)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            (self.table::<Inspectable>().release)(self.0);
        }
    }
}

/// Called only by the CLI's isolated, read-only probe command. A fresh process
/// is required after recovery: the client manager itself caches its snapshot.
pub fn enabled() -> io::Result<Vec<String>> {
    // SAFETY: this child owns its STA; RAII objects release before the apartment.
    hresult(unsafe { RoInitialize(0) })?;
    let _apartment = Apartment;
    let name: Vec<u16> = CLASS.encode_utf16().collect();
    let mut class = ptr::null_mut();
    hresult(unsafe { WindowsCreateString(name.as_ptr(), name.len() as u32, &mut class) })?;
    let class = HString(class);
    let mut raw = ptr::null_mut();
    contract(
        unsafe { RoGetActivationFactory(class.0, &GUID::from_u128(IID_STATICS), &mut raw) },
        || format!("{CLASS} statics {}", braced(IID_STATICS)),
    )?;
    let factory = Com::new(raw)?;
    let manager = factory
        .object(unsafe { factory.table::<Statics>().get_instance })?
        .query(IID_MANAGER, "ICoreKeyboardInputProfileManager")?;
    // The initial remote snapshot arrives asynchronously on the STA. Empty
    // before that message arrives is not evidence of a stale ctfmon cache.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    let profiles = loop {
        let mut message: MSG = unsafe { std::mem::zeroed() };
        unsafe {
            while PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        let profiles = manager.object(unsafe { manager.table::<Manager>().enabled })?;
        let mut count = 0;
        hresult(unsafe { (profiles.table::<Vector>().size)(profiles.0, &mut count) })?;
        if count > 0 {
            break profiles;
        }
        if std::time::Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Windows did not deliver its live input profiles",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let table = unsafe { profiles.table::<Vector>() };
    let mut count = 0;
    hresult(unsafe { (table.size)(profiles.0, &mut count) })?;
    if count > 256 {
        return Err(io::Error::other("unexpected live profile count"));
    }
    let mut result = Vec::new();
    for index in 0..count {
        let mut raw = ptr::null_mut();
        hresult(unsafe { (table.get_at)(profiles.0, index, &mut raw) })?;
        let profile = Com::new(raw)?.query(IID_PROFILE, "ICoreKeyboardInputProfile")?;
        let mut raw = ptr::null_mut();
        hresult(unsafe { (profile.table::<Profile>().as_tip_string)(profile.0, &mut raw) })?;
        let tip = HString(raw);
        let mut length = 0;
        let value = unsafe { WindowsGetStringRawBuffer(tip.0, &mut length) };
        if length == 0 || length > 256 || value.is_null() {
            return Err(io::Error::other("invalid live input profile ID"));
        }
        result.push(
            String::from_utf16(unsafe { std::slice::from_raw_parts(value, length as usize) })
                .map_err(io::Error::other)?,
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_missing_class_or_interface_is_unsupported() {
        for hr in [E_NOINTERFACE, REGDB_E_CLASSNOTREG] {
            let error = contract(hr, || "ICoreKeyboardInputProfile".into()).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::Unsupported);
            assert!(
                error
                    .to_string()
                    .starts_with("ICoreKeyboardInputProfile unavailable: ")
            );
        }
        // E_FAIL, E_ACCESSDENIED, RPC_E_DISCONNECTED and E_POINTER are failures.
        for hr in [0x8000_4005u32, 0x8007_0005, 0x8001_0108, 0x8000_4003] {
            let error = contract(hr as i32, || panic!("not a missing contract")).unwrap_err();
            assert_ne!(error.kind(), io::ErrorKind::Unsupported);
        }
        contract(0, || panic!("success")).unwrap();
        contract(1, || panic!("success")).unwrap();
    }
}
