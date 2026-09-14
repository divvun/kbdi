//! Read the same serialized input profiles consumed by Explorer's switcher.
//!
//! This is a private WinRT contract, isolated here and invoked only in a bounded
//! child process. These IIDs/vtables were checked against Windows 11 26100.9278
//! public symbols. No process memory offsets, DLL patches, or debugger needed.
//! Missing interfaces must fail explicitly, never mean "healthy" or "stale".
use super::native::hresult;
use std::{ffi::c_void, io, ptr};
use windows_core::GUID;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
};

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
    fn from_result(hr: i32, raw: Raw) -> io::Result<Self> {
        hresult(hr)?;
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
    fn query(&self, iid: u128) -> io::Result<Self> {
        let mut raw = ptr::null_mut();
        let hr =
            unsafe { (self.table::<Inspectable>().query)(self.0, &GUID::from_u128(iid), &mut raw) };
        Self::from_result(hr, raw)
    }
    fn object(&self, get: GetObject) -> io::Result<Self> {
        let mut raw = ptr::null_mut();
        Self::from_result(unsafe { get(self.0, &mut raw) }, raw)
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
    let name: Vec<u16> = "Windows.UI.Internal.Text.Core.CoreKeyboardInputProfileManager"
        .encode_utf16()
        .collect();
    let mut class = ptr::null_mut();
    hresult(unsafe { WindowsCreateString(name.as_ptr(), name.len() as u32, &mut class) })?;
    let class = HString(class);
    let mut raw = ptr::null_mut();
    let factory = Com::from_result(
        unsafe {
            RoGetActivationFactory(
                class.0,
                &GUID::from_u128(0xd0380bbe_201e_4a70_8eff_bb52f7996cae),
                &mut raw,
            )
        },
        raw,
    )?;
    let manager = factory
        .object(unsafe { factory.table::<Statics>().get_instance })?
        .query(0xb3b6947d_7825_46a6_82a8_496216b9dd28)?;
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
        let profile =
            Com::from_result(unsafe { (table.get_at)(profiles.0, index, &mut raw) }, raw)?
                .query(0x29fc1d95_e12c_43d9_98a5_de79a87ea36b)?;
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
