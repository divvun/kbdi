extern crate winapi;
extern crate advapi32;
extern crate user32;
extern crate kernel32;
extern crate winreg;

use winapi::{LPWSTR, DWORD, LPARAM, WCHAR};
use winreg::RegKey;
use winreg::enums::*;
use winreg::types::{ToRegValue, FromRegValue};
use std::ptr::null_mut;
use std::io::Error;
use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;

// fn get_keyboard_layout_list() -> Result<Vec<HKL>, Error> {
//     let length = 1024i32;

//     unsafe {
//         let mut handles: Vec<HKL> = vec![null_mut(); length as usize];
//         let ret = user32::GetKeyboardLayoutList(length, handles.as_mut_ptr());

//         if ret == 0 {
//             return Err(Error::last_os_error())
//         }

//         handles.truncate(ret as usize);
//         Ok(handles.into_iter().collect())
//     }
// }

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

fn user_profile() -> RegKey {
    RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Control Panel\\International\\User Profile", KEY_READ | KEY_WRITE)
            .unwrap()
}

pub fn enabled_languages() -> String {
    let user_profile = user_profile();
    user_profile.get_value("Languages").unwrap()
}

pub fn enable_language(language_code: &str) {
    let langs = enabled_languages();
    let mut languages: Vec<&str> = langs.split("\n").collect();
    
    if languages.contains(&language_code) {
        return
    }

    languages.push(language_code);

    // Argh hack
    let s = languages.join("\u{0}");
    let mut regv = s.to_reg_value();
    regv.vtype = REG_MULTI_SZ;
    user_profile().set_raw_value("Languages", &regv).unwrap();
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

pub fn system_locales() -> Vec<String> {
    unsafe extern "system" fn callback(locale: LPWSTR, _: DWORD, l_param: LPARAM) -> i32 {
        let s = lpwstr_to_string(locale);

        let vec = l_param as *mut Vec<String>;
        (*vec).push(s);
        1
    }

    let raw_vec = Box::into_raw(Box::new(vec![]));

    unsafe {
        kernel32::EnumSystemLocalesEx(Some(callback), 0, raw_vec as LPARAM, null_mut());
        *Box::from_raw(raw_vec)
    }
}

fn locale_name_to_lcid(locale_name: &str) -> Result<u32, Error> {
    let loc_name: Vec<u16> = OsStr::new(locale_name)
        .encode_wide()
        .chain(once(0))
        .collect();
    
    unsafe {
        let ret = kernel32::LocaleNameToLCID(loc_name.as_ptr(), 0);

        match ret {
            0 => Err(Error::last_os_error()),
            _ => Ok(ret)
        }
    }
}

pub struct LanguageRegKey {
    id: String,
    regkey: RegKey
}

#[allow(dead_code)]
pub struct KeyboardRegKey {
    id: String,
    regkey: RegKey
}

impl KeyboardRegKey {
    // fn find_by_product_code(product_code: &str) -> Option<KeyboardRegKey> {
    //     let regkey = keyboard_layouts_regkey();
    //     let keys: Vec<String> = regkey.enum_keys().map(|x| x.unwrap()).collect();

    //     for key in keys.into_iter() {
    //         let kl_key = regkey.open_subkey_with_flags(&key, KEY_READ | KEY_WRITE).unwrap();
    //         let ret: Result<String, Error> = kl_key.get_value("Layout Product Code");

    //         if let Ok(v) = ret {
    //             if v == product_code {
    //                 return Some(KeyboardRegKey { id: key.clone(), regkey: kl_key })
    //             }
    //         }
    //     }

    //     None
    // }

    // fn remove_by_product_code(product_code: &str) {
    //     if let Some(kl) = KeyboardRegKey::find_by_product_code(product_code) {
    //         keyboard_layouts_regkey().delete_subkey_all(kl.name()).unwrap();
    //     }
    // }

    fn name(&self) -> &str {
        &self.id
    }

    fn create(
        language_code: &str,
        language_name: &str,
        product_code: &str,
        layout_file: &str,
        layout_name: &str
    ) -> KeyboardRegKey {
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

        KeyboardRegKey { id: key_name.clone(), regkey: regkey }
    }
}

impl LanguageRegKey {
    fn next_transient_lang_id() -> u32 {
        let regkey = user_profile();

        regkey.enum_keys()
            .map(|x| regkey.open_subkey(x.unwrap()))
            .fold(0x2000u32, |acc, x| {
                if let Ok(lang_id) = x.unwrap().get_value::<u32, _>("TransientLangId") {
                    if lang_id >= acc {
                        lang_id + 0x400
                    } else {
                        acc
                    }
                } else {
                    acc
                }
            })
    }

    fn next_layout_order(&self) -> u32 {
        self.regkey.enum_values()
            .fold(1u32, |acc, x| {
                let (k, v) = x.unwrap();

                if k.contains(":") {
                    if let Ok(vv) = u32::from_reg_value(&v) {
                        if vv >= acc {
                            return acc + 1;
                        }
                    }
                }

                acc
            })
    }

    fn set_language_name(&mut self, name: &str) {
        self.regkey.set_value("CachedLanguageName", &name).unwrap();
    }

    // fn language_name(&self) -> Option<String> {
    //     if let Ok(value) = self.regkey.get_value("CachedLanguageName") {
    //         Some(value)
    //     } else {
    //         None
    //     }
    // }

    fn set_transient_lang_id(&mut self, id: u32) {
        self.regkey.set_value("TransientLangId", &id).unwrap();
    }

    fn transient_lang_id(&self) -> Option<u32> {
        if let Ok(value) = self.regkey.get_value("TransientLangId") {
            Some(value)
        } else {
            None
        }
    }

    fn add_keyboard(&mut self, keyboard: KeyboardRegKey) {
        let lcid = if let Some(v) = self.transient_lang_id() {
            v
        } else {
            locale_name_to_lcid(self.id.split(r"\").last().unwrap()).unwrap()
        } as u16;
        let kbd_id = keyboard.name();

        // Add keyboard id to reg
        self.regkey.set_value(format!("{:04X}:{}", &lcid, &kbd_id.to_uppercase()), &self.next_layout_order()).unwrap();

        // Get sub id
        let sub_id = format!("{:08x}", next_substitute_id(lcid));
        
        // Create substitute entry
        kbd_layout_sub_regkey(true).set_value(&sub_id, &kbd_id).unwrap();
        kbd_layout_sub_regkey(false).set_value(&sub_id, &kbd_id).unwrap();

        // Create preload entry
        kbd_layout_preload_regkey(true).set_value(format!("{}", next_preload_id(true)), &sub_id).unwrap();
        kbd_layout_preload_regkey(false).set_value(format!("{}", next_preload_id(false)), &sub_id).unwrap();
    }

    pub fn create(alpha_3_code: &str, native_name: &str) -> LanguageRegKey {
        if let Some(lang_regkey) = LanguageRegKey::find_by_alpha_3_code(&alpha_3_code) {
            return lang_regkey;
        }

        let mut regkey = LanguageRegKey {
            id: alpha_3_code.to_owned(),
            regkey: RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey_with_flags(format!(r"Control Panel\International\User Profile\{}", &alpha_3_code), KEY_READ | KEY_WRITE)
                .unwrap()
        };

        if !system_locales().contains(&alpha_3_code.to_owned()) {
            regkey.set_language_name(native_name);
            regkey.set_transient_lang_id(LanguageRegKey::next_transient_lang_id());
        }

        regkey
    }

    fn find_by_alpha_3_code(alpha_3_code: &str) -> Option<LanguageRegKey> {
        let maybe_regkey = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(format!(r"Control Panel\International\User Profile\{}", &alpha_3_code), KEY_READ | KEY_WRITE);

        if let Ok(regkey) = maybe_regkey {
            Some(LanguageRegKey { id: alpha_3_code.to_owned(), regkey: regkey })
        } else {
            None
        }
    }

}

fn base_regkey(is_all_users: bool) -> RegKey {
    match is_all_users {
        true => {
            RegKey::predef(HKEY_USERS)
                .open_subkey_with_flags(r".DEFAULT", KEY_READ | KEY_WRITE)
                .unwrap()
        },
        false => RegKey::predef(HKEY_CURRENT_USER)
    }
}

fn kbd_layout_sub_regkey(is_all_users: bool) -> RegKey {
    base_regkey(is_all_users)
        .open_subkey_with_flags(r"Keyboard Layout\Substitutes", KEY_READ | KEY_WRITE)
        .unwrap()
}

fn kbd_layout_preload_regkey(is_all_users: bool) -> RegKey {
    base_regkey(is_all_users)
        .open_subkey_with_flags(r"Keyboard Layout\Preload", KEY_READ | KEY_WRITE)
        .unwrap()
}

/// Substitute IDs begin with 0000, then increment to d001, and continue incrementing dXXX.
fn next_substitute_id(suffix: u16) -> u32 {
    let prefix: u16 = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(r"Keyboard Layout\Substitutes", KEY_READ)
        .unwrap()
        .enum_values()
        .fold(0u16, |acc, x| {
            let (k, _) = x.unwrap();

            if let Ok(kk) = u32::from_str_radix(&k, 16) {
                if (kk as u16) == suffix {
                    // Move high bits down
                    let v = (kk >> 16) as u16;

                    if v >= acc {
                        return if v == 0 {
                            0xd001
                        } else {
                            v + 1
                        }
                    }
                }

                acc
            } else {
                acc
            }
        });

    ((prefix as u32) << 16) + (suffix as u32)
}

fn next_preload_id(is_all_users: bool) -> u32 {
    base_regkey(is_all_users)
        .open_subkey_with_flags(r"Keyboard Layout\Preload", KEY_READ)
        .unwrap()
        .enum_values()
        .fold(1u32, |acc, x| {
            let (k, _) = x.unwrap();

            if let Ok(v) = u32::from_str_radix(&k, 10) {
                if v >= acc {
                    v + 1
                } else {
                    acc
                }
            } else {
                acc
            }
        })
}

#[test]
fn test_sub_id() {
    println!("sub_id: {:08x}", next_substitute_id(0xabcd));
    println!("sub_id: {:08x}", next_substitute_id(0x0c09));
}

#[test]
fn test_next_preload_id() {
    println!("preload_id: {}", next_preload_id());
}

#[test]
fn test_it_doth_work() {
    let v = LanguageRegKey::next_transient_lang_id();

    println!("Transient id: {:04x}", v);
}

pub fn install_keyboard(
    language_name: &str,
    product_code: &str,
    layout_file: &str,
    layout_name: &str,
    language_code: &str
) {
    let mut lang = LanguageRegKey::create(language_code, language_name);
    enable_language(language_code);

    let kbd = KeyboardRegKey::create(language_code, language_name, product_code, layout_file, layout_name);
    lang.add_keyboard(kbd);
}
