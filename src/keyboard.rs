use crate::platform::*;
use crate::types::InputList;
use registry::{Data, Hive, RegKey, Security};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::io;
use std::path::Path;

#[cfg(feature = "legacy")]
pub use crate::keyboard_legacy::*;
#[cfg(not(feature = "legacy"))]
pub use crate::keyboard_win8::*;

pub struct KeyboardRegKey {
    id: String,
    regkey: RegKey,
}

#[derive(Debug)]
pub enum Error {
    AlreadyExists,
    NotFound,
    IoError(io::Error),
    RegErr(registry::key::Error),
}

pub fn install(
    tag: &str,
    layout_name: &str,
    product_code: &str,
    layout_file: &str,
    display_name: Option<&str>,
) -> Result<(), Error> {
    log::info!("Checking if already installed");
    if let Some(_) = KeyboardRegKey::find_by_product_code(product_code) {
        return Err(Error::AlreadyExists);
    }

    log::info!("Checking language name is valid");
    let lang_name = match display_name {
        Some(v) => v.to_owned(),
        #[cfg(not(feature = "legacy"))]
        None => winlangdb::get_language_names(tag).unwrap().name,
        #[cfg(feature = "legacy")]
        None => layout_name.to_owned(),
    };

    log::info!("Creating registry key");
    KeyboardRegKey::create(tag, &lang_name, product_code, layout_file, layout_name);
    Ok(())
}

#[cfg(feature = "legacy")]
fn enabled_input_methods() -> InputList {
    InputList::try_from("".to_owned()).unwrap()
}

fn delete_keyboard_regkey(record: KeyboardRegKey) -> Result<(), Error> {
    let klrk = keyboard_layouts_regkey_write();
    match klrk.delete(record.regkey_id(), true) {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::RegErr(e)),
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

fn keyboard_layouts_regkey_readonly() -> RegKey {
    Hive::LocalMachine
        .open(
            r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts",
            Security::Read,
        )
        .unwrap()
}

fn keyboard_layouts_regkey_write() -> RegKey {
    Hive::LocalMachine
        .open(
            r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts",
            Security::Write | Security::Read,
        )
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
            None => continue,
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
            None => continue,
        };

        if !Path::new(r"C:\Windows\System32").join(layout_file).exists() {
            delete_keyboard_regkey(key).unwrap();
        }
    }
}

fn first_available_keyboard_regkey_id(lcid: &str) -> String {
    let regkey = keyboard_layouts_regkey_readonly();
    let mut kbd_keys: Vec<u16> = regkey
        .keys()
        .map(|x| x.unwrap().to_string())
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
    let regkey = keyboard_layouts_regkey_readonly();
    let kbd_keys: Vec<String> = regkey.keys().map(|x| x.unwrap().to_string()).collect();

    let mut layout_ids: Vec<u32> = kbd_keys
        .into_iter()
        .map(|key| {
            let kbdkey = &regkey.open(key, Security::Read | Security::Write).unwrap();
            let layout_idstr: String = match kbdkey.value("Layout Id") {
                Ok(Data::String(v)) => v.to_string_lossy(),
                _ => "0".to_string(),
            };

            u32::from_str_radix(&layout_idstr, 16).unwrap_or(0u32)
        })
        .collect();

    layout_ids.sort();

    format!("{:04x}", layout_ids.last().unwrap() + 1)
}

impl KeyboardRegKey {
    pub fn find_by_product_code(product_code: &str) -> Option<KeyboardRegKey> {
        let regkey = keyboard_layouts_regkey_readonly();
        let keys: Vec<String> = regkey.keys().map(|x| x.unwrap().to_string()).collect();
        for key in keys.into_iter() {
            let kl_key = regkey.open(&key, Security::Read).unwrap();
            let ret: Result<Data, registry::value::Error> = kl_key.value("Layout Product Code");
            match ret {
                Ok(Data::String(s)) if s.to_string_lossy() == product_code => {
                    return Some(KeyboardRegKey {
                        id: key.clone(),
                        regkey: kl_key,
                    })
                }
                _ => continue,
            }
        }

        None
    }

    pub fn installed() -> Vec<KeyboardRegKey> {
        let regkey = keyboard_layouts_regkey_readonly();
        regkey
            .keys()
            .map(|x| x.unwrap().to_string())
            .filter(|x| x.starts_with("a"))
            .map(|x| {
                let k = regkey.open(&x, Security::Read | Security::Write).unwrap();
                KeyboardRegKey {
                    id: x.to_owned(),
                    regkey: k,
                }
            })
            .collect()
    }

    pub fn regkey_id(&self) -> &str {
        &self.id
    }

    pub fn id(&self) -> Option<String> {
        match self.regkey.value("Layout Id") {
            Ok(Data::String(v)) => Some(v.to_string_lossy()),
            _ => None,
        }
    }

    pub fn product_code(&self) -> Option<String> {
        match self.regkey.value("Layout Product Code") {
            Ok(Data::String(v)) => Some(v.to_string_lossy()),
            _ => None,
        }
    }

    pub fn language_name(&self) -> Option<String> {
        match self.regkey.value("Custom Language Name") {
            Ok(Data::String(v)) => Some(v.to_string_lossy()),
            _ => None,
        }
    }

    pub fn layout_file(&self) -> Option<String> {
        match self.regkey.value("Layout File") {
            Ok(Data::String(v)) => Some(v.to_string_lossy()),
            _ => None,
        }
    }

    pub fn layout_name(&self) -> Option<String> {
        match self.regkey.value("Layout Text") {
            Ok(Data::String(v)) => Some(v.to_string_lossy()),
            _ => None,
        }
    }

    pub fn create(
        tag: &str,
        display_name: &str,
        product_code: &str,
        layout_file: &str,
        layout_name: &str,
    ) -> KeyboardRegKey {
        info!("Locale name to lcid");
        let lcid = format!("{:04x}", crate::lcid(&tag) as u16);

        info!("Using lcid '{}'", lcid);

        info!("D: Get first available reg ids");
        let key_name = first_available_keyboard_regkey_id(&lcid);
        let layout_id = first_available_layout_id();

        info!("D: open regkey");
        let regkey = keyboard_layouts_regkey_write()
            .create(&key_name, Security::Read | Security::Write)
            .unwrap();

        info!("D: set regkey vals");
        regkey
            .set_value(
                "Custom Language Display Name",
                &Data::String(
                    format!("@%SystemRoot%\\system32\\{},-1100", &layout_file)
                        .try_into()
                        .unwrap(),
                ),
            )
            .unwrap();
        regkey
            .set_value(
                "Custom Language Name",
                &Data::String(display_name.try_into().unwrap()),
            )
            .unwrap();
        regkey
            .set_value(
                "Layout Display Name",
                &Data::String(
                    format!("@%SystemRoot%\\system32\\{},-1000", &layout_file)
                        .try_into()
                        .unwrap(),
                ),
            )
            .unwrap();
        regkey
            .set_value(
                "Layout File",
                &Data::String(layout_file.try_into().unwrap()),
            )
            .unwrap();
        regkey
            .set_value("Layout Id", &Data::String(layout_id.try_into().unwrap()))
            .unwrap();
        regkey
            .set_value("Layout Locale Name", &Data::String(tag.try_into().unwrap()))
            .unwrap();
        regkey
            .set_value(
                "Layout Product Code",
                &Data::String(product_code.try_into().unwrap()),
            )
            .unwrap();
        regkey
            .set_value(
                "Layout Text",
                &Data::String(layout_name.try_into().unwrap()),
            )
            .unwrap();

        KeyboardRegKey {
            id: key_name.clone(),
            regkey,
        }
    }
}

impl fmt::Display for KeyboardRegKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Registry Key:   {}", self.regkey_id())?;
        writeln!(
            f,
            "Layout Name:    {}",
            self.layout_name().unwrap_or("".to_string())
        )?;
        writeln!(
            f,
            "Language Name:  {}",
            self.language_name().unwrap_or("".to_string())
        )?;
        writeln!(
            f,
            "Layout File:    {}",
            self.layout_file().unwrap_or("".to_string())
        )?;
        writeln!(f, "Layout Id:      {}", self.id().unwrap_or("".to_string()))?;
        writeln!(
            f,
            "Product Code:   {}",
            self.product_code().unwrap_or("".to_string())
        )?;

        Ok(())
    }
}
