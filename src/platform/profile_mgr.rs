//! `ITfInputProcessorProfileMgr`, the documented TSF API that registers and
//! unregisters a text service's language profiles. Profiles are written by
//! TSF itself; kbdi never writes `CTF\TIP` keys.
use super::native::hresult;
use std::{ffi::c_void, io, ptr};
use windows_sys::{
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
    },
    core::GUID,
};

const CLSID_TF_INPUT_PROCESSOR_PROFILES: u128 = 0x33c53a50_f456_4884_b049_85fd643ecfed;
const IID_ITF_INPUT_PROCESSOR_PROFILE_MGR: u128 = 0x71c6e74c_0f28_11d8_a82a_00065b84435c;

type Raw = *mut c_void;

#[repr(C)]
struct Vtbl {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(Raw) -> u32,
    activate_profile: usize,
    deactivate_profile: usize,
    get_profile: usize,
    enum_profiles: usize,
    release_input_processor: usize,
    #[allow(clippy::type_complexity)]
    register_profile: unsafe extern "system" fn(
        Raw,
        *const GUID,
        u16,
        *const GUID,
        *const u16,
        u32,
        *const u16,
        u32,
        u32,
        *mut c_void,
        u32,
        i32,
        u32,
    ) -> i32,
    unregister_profile: unsafe extern "system" fn(Raw, *const GUID, u16, *const GUID, u32) -> i32,
}

/// COM for the calling thread, uninitialized on drop when this call
/// initialized it.
struct Apartment(bool);
impl Apartment {
    fn enter() -> io::Result<Self> {
        // SAFETY: no reserved pointer; balanced by Drop when it succeeded.
        let hr = unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) };
        if hr == RPC_E_CHANGED_MODE {
            // Already a multithreaded apartment, which TSF's profile manager
            // also supports. Not ours to uninitialize.
            return Ok(Self(false));
        }
        hresult(hr)?;
        Ok(Self(true))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful CoInitializeEx above.
            unsafe { CoUninitialize() }
        }
    }
}

pub struct ProfileMgr {
    raw: Raw,
    _apartment: Apartment,
}

impl ProfileMgr {
    pub fn new() -> io::Result<Self> {
        let apartment = Apartment::enter()?;
        let mut raw = ptr::null_mut();
        // SAFETY: valid CLSID/IID and a writable output pointer.
        hresult(unsafe {
            CoCreateInstance(
                &GUID::from_u128(CLSID_TF_INPUT_PROCESSOR_PROFILES),
                ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &GUID::from_u128(IID_ITF_INPUT_PROCESSOR_PROFILE_MGR),
                &mut raw,
            )
        })?;
        if raw.is_null() {
            return Err(io::Error::other("null ITfInputProcessorProfileMgr"));
        }
        Ok(Self {
            raw,
            _apartment: apartment,
        })
    }

    fn vtbl(&self) -> &Vtbl {
        // SAFETY: `raw` is the ITfInputProcessorProfileMgr queried above.
        unsafe { &**self.raw.cast::<*const Vtbl>() }
    }

    /// Registers an enabled-by-default profile with no substitute layout.
    pub fn register(
        &self,
        clsid: u128,
        lang_id: u16,
        profile: u128,
        description: &str,
        icon_file: &str,
    ) -> io::Result<()> {
        let description: Vec<u16> = description.encode_utf16().collect();
        let icon_file: Vec<u16> = icon_file.encode_utf16().collect();
        // SAFETY: the counted strings outlive the call; GUIDs are borrowed.
        hresult(unsafe {
            (self.vtbl().register_profile)(
                self.raw,
                &GUID::from_u128(clsid),
                lang_id,
                &GUID::from_u128(profile),
                description.as_ptr(),
                description.len() as u32,
                icon_file.as_ptr(),
                icon_file.len() as u32,
                0,
                ptr::null_mut(),
                0,
                1,
                0,
            )
        })
    }

    pub fn unregister(&self, clsid: u128, lang_id: u16, profile: u128) -> io::Result<()> {
        // SAFETY: GUIDs are borrowed for the duration of the call.
        hresult(unsafe {
            (self.vtbl().unregister_profile)(
                self.raw,
                &GUID::from_u128(clsid),
                lang_id,
                &GUID::from_u128(profile),
                0,
            )
        })
    }
}

impl Drop for ProfileMgr {
    fn drop(&mut self) {
        // SAFETY: releases the reference CoCreateInstance returned.
        unsafe {
            (self.vtbl().release)(self.raw);
        }
    }
}
