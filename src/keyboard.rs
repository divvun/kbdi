use std::io;
use winreg::*;
use winreg::enums::*;
use platform::*;
use std::fmt;

pub struct KeyboardRegKey {
    id: String,
    regkey: RegKey
}

pub enum Error {
    AlreadyExists,
    NotFound,
    IoError(io::Error)
}

pub fn install(
    tag: &str,
    display_name: &str,
    product_code: &str,
    layout_file: &str,
    layout_name: &str
) -> Result<(), Error> {
    if let Some(_) = KeyboardRegKey::find_by_product_code(product_code) {
        return Err(Error::AlreadyExists);
    }

    KeyboardRegKey::create(tag, display_name, product_code, layout_file, layout_name);
    Ok(())
}

pub fn uninstall(product_code: &str) -> Result<(), Error> {
    if let Some(record) = KeyboardRegKey::find_by_product_code(product_code) {
        return match record.regkey.delete_subkey_all(record.regkey_id()) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::IoError(e))
        }; 
    }

    Err(Error::NotFound)
}

pub fn installed() -> Vec<KeyboardRegKey> {
    KeyboardRegKey::installed()
}

fn keyboard_layouts_regkey() -> RegKey {
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts", KEY_READ | KEY_WRITE)
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

impl KeyboardRegKey {
    fn find_by_product_code(product_code: &str) -> Option<KeyboardRegKey> {
        let regkey = keyboard_layouts_regkey();
        let keys: Vec<String> = regkey.enum_keys().map(|x| x.unwrap()).collect();
        for key in keys.into_iter() {
            let kl_key = regkey.open_subkey_with_flags(&key, KEY_READ | KEY_WRITE).unwrap();
            let ret: Result<String, io::Error> = kl_key.get_value("Layout Product Code");
            if let Ok(v) = ret {
                if v == product_code {
                    return Some(KeyboardRegKey { id: key.clone(), regkey: kl_key })
                }
            }
        }
        None
    }

    fn installed() -> Vec<KeyboardRegKey> {
        let regkey = keyboard_layouts_regkey();
        regkey.enum_keys()
            .map(|x| x.unwrap())
            .filter(|x| x.starts_with("a"))
            .map(|x| {
                let k = regkey.open_subkey_with_flags(&x, KEY_READ | KEY_WRITE).unwrap();
                KeyboardRegKey { id: x.to_owned(), regkey: k }
            })
            .collect()
    }

    pub fn regkey_id(&self) -> &str {
        &self.id
    }

    pub fn id(&self) -> Option<String> {
        match self.regkey.get_value("Layout Id") {
            Ok(v) => Some(v),
            Err(_) => None
        }
    }

    pub fn product_code(&self) -> Option<String> {
        match self.regkey.get_value("Layout Product Code") {
            Ok(v) => Some(v),
            Err(_) => None
        }
    }
    
    pub fn language_name(&self) -> Option<String> {
        match self.regkey.get_value("Custom Language Name") {
            Ok(v) => Some(v),
            Err(_) => None
        }
    }

    pub fn layout_file(&self) -> Option<String> {
        match self.regkey.get_value("Layout File") {
            Ok(v) => Some(v),
            Err(_) => None
        }
    }

    pub fn layout_name(&self) -> Option<String> {
        match self.regkey.get_value("Layout Text") {
            Ok(v) => Some(v),
            Err(_) => None
        }
    }

    fn create(
        tag: &str,
        display_name: &str,
        product_code: &str,
        layout_file: &str,
        layout_name: &str
    ) -> KeyboardRegKey {
        let lcid = format!("{:04x}", winnls::locale_name_to_lcid(&tag).unwrap() as u16);

        let key_name = first_available_keyboard_regkey_id(&lcid);
        let layout_id = first_available_layout_id();

        let regkey = keyboard_layouts_regkey()
                .create_subkey_with_flags(&key_name, KEY_READ | KEY_WRITE)
                .unwrap();

        regkey.set_value("Custom Language Display Name",
            &format!("@%SystemRoot%\\system32\\{},-1100", &layout_file).to_string()).unwrap();
        regkey.set_value("Custom Language Name", &display_name).unwrap();
        regkey.set_value("Layout Display Name", 
            &format!("@%SystemRoot%\\system32\\{},-1000", &layout_file).to_string()).unwrap();
        regkey.set_value("Layout File", &layout_file).unwrap();
        regkey.set_value("Layout Id", &layout_id).unwrap();
        regkey.set_value("Layout Product Code", &product_code).unwrap();
        regkey.set_value("Layout Text", &layout_name).unwrap();

        KeyboardRegKey { id: key_name.clone(), regkey: regkey }
    }
}

impl fmt::Display for KeyboardRegKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Registry Key:   {}", self.regkey_id());
        writeln!(f, "Layout Name:    {}", self.layout_name().unwrap_or("".to_string()));
        writeln!(f, "Language Name:  {}", self.language_name().unwrap_or("".to_string()));
        writeln!(f, "Layout File:    {}", self.layout_file().unwrap_or("".to_string()));
        writeln!(f, "Layout Id:      {}", self.id().unwrap_or("".to_string()));
        writeln!(f, "Product Code:   {}", self.product_code().unwrap_or("".to_string()));
        
        Ok(())
    }
}

// fn kbd_layout_sub_regkey(is_all_users: bool) -> RegKey {
//     base_regkey(is_all_users)
//         .open_subkey_with_flags(r"Keyboard Layout\Substitutes", KEY_READ | KEY_WRITE)
//         .unwrap()
// }

// fn kbd_layout_preload_regkey(is_all_users: bool) -> RegKey {
//     base_regkey(is_all_users)
//         .open_subkey_with_flags(r"Keyboard Layout\Preload", KEY_READ | KEY_WRITE)
//         .unwrap()
// }

// /// Substitute IDs begin with 0000, then increment to d001, and continue incrementing dXXX.
// fn next_substitute_id(suffix: u16) -> u32 {
//     let prefix: u16 = RegKey::predef(HKEY_CURRENT_USER)
//         .open_subkey_with_flags(r"Keyboard Layout\Substitutes", KEY_READ)
//         .unwrap()
//         .enum_values()
//         .fold(0u16, |acc, x| {
//             let (k, _) = x.unwrap();

//             if let Ok(kk) = u32::from_str_radix(&k, 16) {
//                 if (kk as u16) == suffix {
//                     // Move high bits down
//                     let v = (kk >> 16) as u16;

//                     if v >= acc {
//                         return if v == 0 {
//                             0xd001
//                         } else {
//                             v + 1
//                         }
//                     }
//                 }

//                 acc
//             } else {
//                 acc
//             }
//         });

//     ((prefix as u32) << 16) + (suffix as u32)
// }

// fn next_preload_id(is_all_users: bool) -> u32 {
//     base_regkey(is_all_users)
//         .open_subkey_with_flags(r"Keyboard Layout\Preload", KEY_READ)
//         .unwrap()
//         .enum_values()
//         .fold(1u32, |acc, x| {
//             let (k, _) = x.unwrap();

//             if let Ok(v) = u32::from_str_radix(&k, 10) {
//                 if v >= acc {
//                     v + 1
//                 } else {
//                     acc
//                 }
//             } else {
//                 acc
//             }
//         })
// }
