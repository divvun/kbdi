use registry::{Hive, Data, RegKey, Security};
use crate::keyboard::{Error, KeyboardRegKey};
use crate::types::*;
use crate::platform::*;
use std::convert::TryFrom;

pub fn enable(tag: &str, product_code: &str) -> Result<(), Error> {
    let record = match KeyboardRegKey::find_by_product_code(product_code) {
        Some(v) => v,
        None => return Err(Error::NotFound)
    };

    // Generate input list item
    let lcid = crate::lcid(tag);
    let tip = InputList::try_from(format!("{:04X}:{}", lcid, record.regkey_id())).unwrap();

    info!("D: Install layout, flag 0");
    input::install_layout(tip, 0x0).unwrap();
    // info!("D: Enable keyboard layout");
    // winuser::load_keyboard_layout(record.regkey_id());
    Ok(())
}

fn base_regkey(is_all_users: bool) -> RegKey {
    match is_all_users {
        true => {
            Hive::Users.open(".DEFAULT", Security::Read | Security::Write).unwrap()
        },
        false => Hive::CurrentUser.open("", Security::Read | Security::Write).unwrap()
    }
}

fn kbd_layout_sub_regkey(is_all_users: bool) -> RegKey {
    base_regkey(is_all_users)
        .open(r"Keyboard Layout\Substitutes", Security::Read | Security::Write)
        .unwrap()
}

fn kbd_layout_preload_regkey(is_all_users: bool) -> RegKey {
    base_regkey(is_all_users)
        .open(r"Keyboard Layout\Preload", Security::Read | Security::Write)
        .unwrap()
}

/// Substitute IDs begin with 0000, then increment to d001, and continue incrementing dXXX.
fn next_substitute_id(suffix: u16) -> u32 {
    let prefix: u16 = Hive::CurrentUser
        .open(r"Keyboard Layout\Substitutes", Security::Read)
        .unwrap()
        .values()
        .fold(0u16, |acc, x| {
            let name = x.unwrap().name().to_string_lossy();

            if let Ok(val) = u32::from_str_radix(&name, 16) {
                if (val as u16) == suffix {
                    // Move high bits down
                    let v = (val >> 16) as u16;

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
        .open(r"Keyboard Layout\Preload", Security::Read)
        .unwrap()
        .values()
        .fold(1u32, |acc, x| {
            let name = x.unwrap().name().to_string_lossy();

            if let Ok(v) = u32::from_str_radix(&name, 10) {
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
