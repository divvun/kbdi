extern crate winapi;
extern crate advapi32;
extern crate user32;
extern crate kernel32;
extern crate winreg;

use winapi::{HKEY, HKL, LPWSTR, DWORD, LPARAM, WCHAR};
use winreg::{RegKey, RegValue};
use winreg::enums::*;
use winreg::types::{ToRegValue};
use std::ptr::null_mut;
use std::io::Error;
use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;

#[derive(Debug, PartialEq, Eq)]
struct KbdLayoutHandle {
    _inner: HKL
}

fn get_keyboard_layout_list() -> Result<Vec<KbdLayoutHandle>, Error> {
    let length = 1024i32;

    unsafe {
        let mut handles: Vec<HKL> = vec![null_mut(); length as usize];
        let ret = user32::GetKeyboardLayoutList(length, handles.as_mut_ptr());

        if ret == 0 {
            return Err(Error::last_os_error())
        }

        handles.truncate(ret as usize);
        Ok(handles.into_iter().map(|x| KbdLayoutHandle { _inner: x }).collect())
    }
}

fn load_keyboard_layout(klid: &str) -> Result<KbdLayoutHandle, Error> {
    let klid_win: Vec<u16> = OsStr::new(klid).encode_wide().chain(once(0)).collect();

    unsafe {
        let ret: HKL = user32::LoadKeyboardLayoutW(klid_win.as_ptr(), winapi::KLF_NOTELLSHELL);

        if ret.is_null() {
            return Err(Error::last_os_error())
        }

        Ok(KbdLayoutHandle { _inner: ret })
    }
}

fn default_keyboard() -> KbdLayoutHandle {
    load_keyboard_layout("").unwrap()
}

fn keyboard_layouts_regkey() -> RegKey {
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags("SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts", KEY_READ | KEY_WRITE)
        .unwrap()
}

fn first_available_keyboard_regkey_id(lcid: &str) -> String {
    let regkey = keyboard_layouts_regkey();
    let mut kbd_keys: Vec<u16> = regkey.enum_keys()
        .map(|x| x.unwrap())
        .filter(|x| x.starts_with(&"a") && x.ends_with(&lcid))
        .map(|x| {
            let n = u32::from_str_radix(&x, 16).unwrap_or(0u32);
            (n >> 16) as u16
        })
        .collect();
    
    kbd_keys.sort();

    if let Some(last) = kbd_keys.last() {
        format!("{:04x}{}", last + 1, lcid)
    } else {
        format!("a000{}", lcid)
    }
}

fn first_available_layout_id() -> String {
    let regkey = keyboard_layouts_regkey();
    let kbd_keys: Vec<String> = regkey.enum_keys().map(|x| x.unwrap()).collect();

    let mut layout_ids: Vec<u32> = kbd_keys.into_iter().map(|key| {
        let kbdkey = &regkey.open_subkey_with_flags(key, KEY_READ | KEY_WRITE).unwrap();
        let layout_idstr: String = kbdkey.get_value("Layout Id").unwrap_or("0".to_string());

        u32::from_str_radix(&layout_idstr, 16).unwrap_or(0u32)
    }).collect();

    layout_ids.sort();
    
    format!("{:04x}", layout_ids.last().unwrap() + 1)
}

fn find_layout_by_product_code(product_code: &str) -> Option<String> {
    let regkey = keyboard_layouts_regkey();
    let keys: Vec<String> = regkey.enum_keys().map(|x| x.unwrap()).collect();

    for key in keys.into_iter() {
        let kl_key = regkey.open_subkey_with_flags(&key, KEY_READ).unwrap();
        let ret: Result<String, Error> = kl_key.get_value("Layout Product Code");

        if let Ok(v) = ret {
            if v == product_code {
                return Some(key)
            }
        }
    }

    None
}

fn create_keyboard_layout_regkey(
    language_code: &str,
    language_name: &str,
    product_code: &str,
    layout_file: &str,
    layout_name: &str
) -> String {
    if let Some(kl) = find_layout_by_product_code(product_code) {
        keyboard_layouts_regkey().delete_subkey_all(kl).unwrap();
    }

    let lcid = format!("{:04x}", locale_name_to_lcid(&language_code).unwrap() as u16);

    let key_name = first_available_keyboard_regkey_id(&lcid);
    let layout_id = first_available_layout_id();

    let regkey = keyboard_layouts_regkey()
            .create_subkey_with_flags(&key_name, KEY_READ | KEY_WRITE)
            .unwrap();

    regkey.set_value("Custom Language Display Name",
        &format!("@%SystemRoot%\\system32\\{},-1100", &layout_file).to_string()).unwrap();
    regkey.set_value("Custom Language Name", &language_name).unwrap();
    regkey.set_value("Layout Display Name", 
        &format!("@%SystemRoot%\\system32\\{},-1000", &layout_file).to_string()).unwrap();
    regkey.set_value("Layout File", &layout_file).unwrap();
    regkey.set_value("Layout Id", &layout_id).unwrap();
    regkey.set_value("Layout Product Code", &product_code).unwrap();
    regkey.set_value("Layout Text", &layout_name).unwrap();

    key_name
}

fn user_profile() -> RegKey {
    RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Control Panel\\International\\User Profile", KEY_READ | KEY_WRITE)
            .unwrap()
}

fn enable_language(language_code: &str) {
    let user_profile = user_profile();

    let lang_string: String = user_profile.get_value::<String, &str>("Languages").unwrap();
    let mut languages: Vec<&str> = lang_string.split("\n").collect();
    
    if languages.contains(&language_code) {
        return
    }

    languages.push(language_code);

    // Argh hack
    let s = languages.join("\u{0}");
    let mut regv = s.to_reg_value();
    regv.vtype = REG_MULTI_SZ;
    user_profile.set_raw_value("Languages", &regv).unwrap();
}

// TODO: use from winapi once supported
const LOCALE_NAME_MAX_LENGTH: usize = 85;

fn resolve_locale(language_code: &str) -> Result<String, Error> {
    let mut buffer: Vec<u16> = vec![0; LOCALE_NAME_MAX_LENGTH];
    let lang_code: Vec<u16> = OsStr::new(language_code).encode_wide().chain(once(0)).collect();

    unsafe {
        let ret = kernel32::ResolveLocaleName(lang_code.as_ptr(), buffer.as_mut_ptr(), LOCALE_NAME_MAX_LENGTH as i32);

         if ret == 0 {
            return Err(Error::last_os_error())
        }

        buffer.truncate((ret - 1) as usize);
        
        Ok(String::from_utf16_lossy(&buffer))
    }
}

unsafe fn lpwstr_to_string(lpw: LPWSTR) -> String {
    let mut buf: Vec<WCHAR> = vec![];
    let mut i = 0isize;

    while *lpw.offset(i) != 0 {
        buf.push(*lpw.offset(i));
        i += 1
    }

    return String::from_utf16_lossy(&buf);
}

fn system_locales() -> Vec<String> {
    let mut locales: Vec<String> = vec![];

    unsafe extern "system" fn callback(locale: LPWSTR, flags: DWORD, l_param: LPARAM) -> i32 {
        //let foo = String::from_utf16_lossy(locale);
        let s = lpwstr_to_string(locale);
        println!("{:?}", &s);
        1
    }

    unsafe {
        kernel32::EnumSystemLocalesEx(Some(callback), 0x00000010, 0, null_mut());
    }

    return locales;
}

fn lcid_to_locale_name(lcid_str: &str) -> Result<String, Error> {
    let mut buffer: Vec<u16> = vec![0; LOCALE_NAME_MAX_LENGTH];
    let lcid = u32::from_str_radix(&lcid_str, 16).unwrap();

    unsafe {
        let ret = kernel32::LCIDToLocaleName(lcid, buffer.as_mut_ptr(), LOCALE_NAME_MAX_LENGTH as i32, 0);

         if ret == 0 {
            return Err(Error::last_os_error())
        }

        buffer.truncate((ret - 1) as usize);
        
        Ok(String::from_utf16_lossy(&buffer))
    }
}

fn locale_name_to_lcid(locale_name: &str) -> Result<u32, Error> {
    let loc_name: Vec<u16> = OsStr::new(locale_name).encode_wide().chain(once(0)).collect();
    unsafe {
        let ret = kernel32::LocaleNameToLCID(loc_name.as_ptr(), 0);

         if ret == 0 {
            return Err(Error::last_os_error())
        }
        
        Ok(ret)
    }
}

#[test]
fn ohno() {
    let lcid = locale_name_to_lcid("sma-Latn-NO").unwrap();
    println!("{:?} {:04X} {:x}", lcid, lcid, lcid);
}

fn system_lcids() {
    unsafe extern "system" fn callback(locale: LPWSTR) -> i32 {
        //let foo = String::from_utf16_lossy(locale);
        let s = lpwstr_to_string(locale);
        let ss = lcid_to_locale_name(&s);
        println!("{:?}", &ss);
        1
    }

    unsafe {
        kernel32::EnumSystemLocalesW(Some(callback), 0);
    }
}

#[test]
fn resolve_locale_test() {
    //system_locales();
    // println!("{:?}", system_locales());
    // println!("{:?}", resolve_locale("sma"));
    // println!("{:?}", resolve_locale("sma-NO"));
    // println!("{:?}", resolve_locale("sma-SE"));
}

fn add_keyboard_to_language(klid: &str, language_code: &str) {
    let user_profile = user_profile();
    let locale_key = user_profile.create_subkey_with_flags(&language_code, KEY_READ | KEY_WRITE).unwrap();
    let lcid = locale_name_to_lcid(&language_code).unwrap() as u16;
    
    let dwords = locale_key.enum_values()
            .map(|x| x.unwrap())
            .filter(|x| x.1.vtype == RegType::REG_DWORD)
            .count() as u32;

    let kl_key = format!("{:04X}:{}", lcid, klid).to_string().to_uppercase();
    locale_key.set_value(&kl_key, &(dwords + 1)).unwrap();
}

fn add_keyboard_to_preload(klid: &str, hkey: HKEY, path: &str) {
    let preload = RegKey::predef(hkey)
            .open_subkey_with_flags(&path, KEY_READ | KEY_WRITE)
            .unwrap();
    
    let count = preload.enum_values()
            .map(|x| x.unwrap())
            .count();
    
    preload.set_value(format!("{}", count + 1), &klid).unwrap();
}

fn add_keyboard_to_preload_all(klid: &str) {
    add_keyboard_to_preload(&klid, HKEY_CURRENT_USER, "Keyboard Layout\\Preload");
    add_keyboard_to_preload(&klid, HKEY_USERS, ".DEFAULT\\Keyboard Layout\\Preload");
    add_keyboard_to_preload(&klid, HKEY_LOCAL_MACHINE, "SYSTEM\\Keyboard Layout\\Preload");
}

pub fn install_keyboard(
    language_name: &str,
    product_code: &str,
    layout_file: &str,
    layout_name: &str,
    language_code: &str
) {
    let klid = &create_keyboard_layout_regkey(language_code, language_name, product_code, layout_file, layout_name);
    enable_language(language_code);
    add_keyboard_to_language(klid, language_code);
    add_keyboard_to_preload_all(klid);
    load_keyboard_layout(klid);
}
