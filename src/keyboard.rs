use std::io;
use winreg::RegKey;
use winreg::enums::{
    HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE
};
use crate::platform::*;
use std::fmt;
use crate::types::*;
use std::path::Path;

#[cfg(not(feature = "legacy"))]
pub use crate::keyboard_win8::*;
#[cfg(feature = "legacy")]
pub use keyboard_legacy::*;

pub struct KeyboardRegKey {
    id: String,
    regkey: RegKey
}

#[derive(Debug)]
pub enum Error {
    AlreadyExists,
    NotFound,
    IoError(io::Error)
}

pub fn install(
    tag: &str,
    layout_name: &str,
    product_code: &str,
    layout_file: &str,
    display_name: Option<&str>
) -> Result<(), Error> {
    println!("D: Checking if already installed");
    if let Some(_) = KeyboardRegKey::find_by_product_code(product_code) {
        return Err(Error::AlreadyExists);
    }

    println!("D: Checking language name is valid");
    let lang_name = match display_name {
        Some(v) => v.to_owned(),
        #[cfg(not(feature = "legacy"))]
        None => winlangdb::get_language_names(tag).unwrap().name,
        #[cfg(feature = "legacy")]
        None => layout_name.to_owned()
    };

    println!("D: Creating registry key");
    KeyboardRegKey::create(tag, &lang_name, product_code, layout_file, layout_name);
    Ok(())
}

#[cfg(feature = "legacy")]
fn enabled_input_methods() -> InputList {
    InputList::from("".to_owned())
}

fn delete_keyboard_regkey(record: KeyboardRegKey) -> Result<(), Error> {
    let klrk = keyboard_layouts_regkey();
    match klrk.delete_subkey_all(record.regkey_id()) {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::IoError(e))
    }
}

pub fn uninstall(product_code: &str) -> Result<(), Error> {
    if let Some(record) = KeyboardRegKey::find_by_product_code(product_code) {
        delete_keyboard_regkey(record)?;
        crate::clean().unwrap();
        return Ok(());
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

pub fn remove_invalid() {
    remove_duplicate_guids();
    remove_invalid_dlls();
    #[cfg(not(feature = "legacy"))]
    remove_invalid_kbids();
}

fn remove_duplicate_guids() {
    // Find duplicate GUIDs, clear all but first
    let mut guids = vec![];
    let keys = KeyboardRegKey::installed();
    for key in keys {
        let guid = match key.product_code() {
            Some(v) => v,
            None => continue
        };

        if guids.contains(&guid) {
            delete_keyboard_regkey(key).unwrap();
        } else {
            guids.push(guid);
        }
    }
}

fn remove_invalid_dlls() {
    let keys = KeyboardRegKey::installed();

    for key in keys {
        let layout_file = match key.layout_file() {
            Some(v) => v,
            None => continue
        };

        if !Path::new(r"C:\Windows\System32").join(layout_file).exists() {
            delete_keyboard_regkey(key).unwrap();
        }

    }
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
    pub fn find_by_product_code(product_code: &str) -> Option<KeyboardRegKey> {
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

    pub fn installed() -> Vec<KeyboardRegKey> {
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

    pub fn create(
        tag: &str,
        display_name: &str,
        product_code: &str,
        layout_file: &str,
        layout_name: &str
    ) -> KeyboardRegKey {
        println!("D: Locale name to lcid");
        let lcid = format!("{:04x}", winnls::locale_name_to_lcid(&tag)
                .unwrap_or(0x0c00) as u16);

        println!("D: Using lcid '{}'", lcid);

        println!("D: Get first available reg ids");
        let key_name = first_available_keyboard_regkey_id(&lcid);
        let layout_id = first_available_layout_id();

        println!("D: open regkey");
        let regkey = keyboard_layouts_regkey()
                .create_subkey_with_flags(&key_name, KEY_READ | KEY_WRITE)
                .unwrap();

        println!("D: set regkey vals");
        regkey.set_value("Custom Language Display Name",
            &format!("@%SystemRoot%\\system32\\{},-1100", &layout_file).to_string()).unwrap();
        regkey.set_value("Custom Language Name", &display_name).unwrap();
        regkey.set_value("Layout Display Name", 
            &format!("@%SystemRoot%\\system32\\{},-1000", &layout_file).to_string()).unwrap();
        regkey.set_value("Layout File", &layout_file).unwrap();
        regkey.set_value("Layout Id", &layout_id).unwrap();
        regkey.set_value("Layout Locale Name", &tag).unwrap();
        regkey.set_value("Layout Product Code", &product_code).unwrap();
        regkey.set_value("Layout Text", &layout_name).unwrap();

        KeyboardRegKey { id: key_name.clone(), regkey: regkey }
    }
}

impl fmt::Display for KeyboardRegKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Registry Key:   {}", self.regkey_id())?;
        writeln!(f, "Layout Name:    {}", self.layout_name().unwrap_or("".to_string()))?;
        writeln!(f, "Language Name:  {}", self.language_name().unwrap_or("".to_string()))?;
        writeln!(f, "Layout File:    {}", self.layout_file().unwrap_or("".to_string()))?;
        writeln!(f, "Layout Id:      {}", self.id().unwrap_or("".to_string()))?;
        writeln!(f, "Product Code:   {}", self.product_code().unwrap_or("".to_string()))?;
        
        Ok(())
    }
}
