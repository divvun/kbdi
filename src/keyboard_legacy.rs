use winreg::RegKey;
use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_USERS, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE
};
use keyboard::{Error, KeyboardRegKey};
use types::*;
use platform::*;

pub fn enable(tag: &str, product_code: &str) -> Result<(), Error> {
    let record = match KeyboardRegKey::find_by_product_code(product_code) {
        Some(v) => v,
        None => return Err(Error::NotFound)
    };

    // Generate input list item
    let lcid = winnls::locale_name_to_lcid(tag).unwrap_or(0x0c00);
    let tip = InputList::from(format!("{:04X}:{}", lcid, record.regkey_id()));

    println!("D: Install layout, flag 0");
    input::install_layout(tip, 0x0).unwrap();
    // println!("D: Enable keyboard layout");
    // winuser::load_keyboard_layout(record.regkey_id());
    Ok(())
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

#[cfg(feature = "legacy")]
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
