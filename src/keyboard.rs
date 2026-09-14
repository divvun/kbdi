#[cfg(not(feature = "legacy"))]
use crate::platform::*;
use crate::registry_snapshot;
#[cfg(feature = "legacy")]
use crate::types::InputList;
#[cfg(feature = "legacy")]
use std::convert::TryFrom;
use std::fmt;
use std::io;
use std::path::Path;
use windows_registry::{Key as RegKey, LOCAL_MACHINE};

#[cfg(feature = "legacy")]
pub use crate::keyboard_legacy::*;
#[cfg(not(feature = "legacy"))]
pub use crate::keyboard_win8::*;

pub struct KeyboardRegKey {
    id: String,
    regkey: RegKey,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("keyboard already installed")]
    AlreadyExists,
    #[error("keyboard not found")]
    NotFound,
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    RegErr(#[from] windows_result::Error),
}

pub fn install(
    tag: &str,
    layout_name: &str,
    product_code: &str,
    layout_file: &str,
    display_name: Option<&str>,
) -> Result<(), Error> {
    #[cfg(not(feature = "legacy"))]
    crate::win8::validate_language_tag(tag)?;

    log::info!("Checking if already installed");
    if let Some(_) = KeyboardRegKey::find_by_product_code(product_code) {
        return Err(Error::AlreadyExists);
    }

    log::info!("Checking language name is valid");
    let lang_name = match display_name {
        Some(v) => v.to_owned(),
        #[cfg(not(feature = "legacy"))]
        None => {
            winlangdb::get_language_names(tag)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unsupported language tag: {tag}"),
                    )
                })?
                .name
        }
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

fn keyboard_layouts_regkey_delete() -> Result<RegKey, Error> {
    // RegDeleteTree additionally requires DELETE and enumeration rights.
    // Request these only for this uninstall operation, not ordinary reads.
    Ok(LOCAL_MACHINE
        .options()
        .access(windows_sys::Win32::System::Registry::KEY_ALL_ACCESS)
        .open(r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts")?)
}

fn delete_keyboard_regkey(record: KeyboardRegKey) -> Result<(), Error> {
    match keyboard_layouts_regkey_delete()?.remove_tree(record.regkey_id()) {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::RegErr(e)),
    }
}

pub fn uninstall(product_code: &str) -> Result<(), Error> {
    if let Some(record) = KeyboardRegKey::find_by_product_code(product_code) {
        // Check machine permissions before changing the current user's input list.
        let layouts = keyboard_layouts_regkey_delete()?;
        #[cfg(not(feature = "legacy"))]
        crate::keyboard_win8::disable_keyboard(record.regkey_id())?;
        layouts.remove_tree(record.regkey_id())?;
        return Ok(());
    }

    // Repeated uninstall is a no-op; never run global cleanup for one product.
    Ok(())
}

pub fn installed() -> Vec<KeyboardRegKey> {
    KeyboardRegKey::installed()
}

fn keyboard_layouts_regkey_readonly() -> RegKey {
    LOCAL_MACHINE
        .open(r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts")
        .unwrap()
}

fn keyboard_layouts_regkey_write() -> RegKey {
    LOCAL_MACHINE
        .options()
        .read()
        .write()
        .open(r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts")
        .unwrap()
}

pub fn remove_invalid() -> Result<(), Error> {
    remove_duplicate_guids()?;
    remove_invalid_dlls()?;
    #[cfg(not(feature = "legacy"))]
    remove_invalid_kbids()?;
    Ok(())
}

fn remove_duplicate_guids() -> Result<(), Error> {
    // Find duplicate GUIDs, clear all but first
    let mut guids = vec![];
    let keys = KeyboardRegKey::installed();
    for key in keys {
        let guid = match key.product_code() {
            Some(v) => v,
            None => continue,
        };

        if guids.contains(&guid) {
            delete_keyboard_regkey(key)?;
        } else {
            guids.push(guid);
        }
    }
    Ok(())
}

fn remove_invalid_dlls() -> Result<(), Error> {
    let keys = KeyboardRegKey::installed();

    for key in keys {
        let layout_file = match key.layout_file() {
            Some(v) => v,
            None => continue,
        };

        if !Path::new(r"C:\Windows\System32").join(layout_file).exists() {
            delete_keyboard_regkey(key)?;
        }
    }
    Ok(())
}

fn first_available_keyboard_regkey_id(lcid: &str) -> String {
    let regkey = keyboard_layouts_regkey_readonly();
    let mut kbd_keys: Vec<u16> = registry_snapshot::keys(&regkey)
        .unwrap()
        .into_iter()
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
    let kbd_keys: Vec<String> = registry_snapshot::keys(&regkey)
        .unwrap()
        .into_iter()
        .collect();

    let mut layout_ids: Vec<u32> = kbd_keys
        .into_iter()
        .map(|key| {
            let kbdkey = &regkey.open(key).unwrap();
            let layout_idstr: String = match kbdkey.get_string("Layout Id") {
                Ok(v) => v,
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
        let keys: Vec<String> = registry_snapshot::keys(&regkey)
            .unwrap()
            .into_iter()
            .collect();
        for key in keys.into_iter() {
            let kl_key = regkey.open(&key).unwrap();
            let ret: windows_registry::Result<String> = kl_key.get_string("Layout Product Code");
            match ret {
                Ok(s) if s == product_code => {
                    return Some(KeyboardRegKey {
                        id: key.clone(),
                        regkey: kl_key,
                    });
                }
                _ => continue,
            }
        }

        None
    }

    pub fn installed() -> Vec<KeyboardRegKey> {
        let regkey = keyboard_layouts_regkey_readonly();
        registry_snapshot::keys(&regkey)
            .unwrap()
            .into_iter()
            .filter(|x| x.starts_with("a"))
            .map(|x| {
                let k = regkey.open(&x).unwrap();
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
        match self.regkey.get_string("Layout Id") {
            Ok(v) => Some(v),
            _ => None,
        }
    }

    pub fn product_code(&self) -> Option<String> {
        match self.regkey.get_string("Layout Product Code") {
            Ok(v) => Some(v),
            _ => None,
        }
    }

    pub fn language_name(&self) -> Option<String> {
        match self.regkey.get_string("Custom Language Name") {
            Ok(v) => Some(v),
            _ => None,
        }
    }

    pub fn layout_file(&self) -> Option<String> {
        match self.regkey.get_string("Layout File") {
            Ok(v) => Some(v),
            _ => None,
        }
    }

    pub fn layout_name(&self) -> Option<String> {
        match self.regkey.get_string("Layout Text") {
            Ok(v) => Some(v),
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
        let regkey = keyboard_layouts_regkey_write().create(&key_name).unwrap();

        for (name, value) in [
            (
                "Custom Language Display Name",
                format!("@%SystemRoot%\\system32\\{},-1100", layout_file),
            ),
            ("Custom Language Name", display_name.to_owned()),
            (
                "Layout Display Name",
                format!("@%SystemRoot%\\system32\\{},-1000", layout_file),
            ),
            ("Layout File", layout_file.to_owned()),
            ("Layout Id", layout_id),
            ("Layout Locale Name", tag.to_owned()),
            ("Layout Product Code", product_code.to_owned()),
            ("Layout Text", layout_name.to_owned()),
        ] {
            regkey.set_string(name, value).unwrap();
        }

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
