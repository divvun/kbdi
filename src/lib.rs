extern crate winapi;
extern crate winreg;

use std::io;
use winrust::*;
use platform::*;

mod types;
mod winrust;
pub mod keyboard;
pub mod platform;

fn set_user_languages(tags: &[String]) -> Result<(), String> {
    let valid_tags: Vec<String> =
        tags.iter()
        .flat_map(|t| winlangdb::get_language_names(t))
        .map(|t| t.tag)
        .collect();

    winlangdb::set_user_languages(&valid_tags)
        .or_else(|_| Err("Failed enabling languages".to_owned()))?;
    winlangdb::remove_inputs_for_all_languages_internal()
        .or_else(|_| Err("Remove inputs failed".to_owned()))?;

    let results: Vec<Result<(), String>> = valid_tags.iter().map(|tag| {
        let lcid = match bcp47langs::lcid_from_bcp47(tag) {
            Some(v) => v,
            None => return Err("Failed to enable tag".to_owned())
        };

        let mut inputs = winlangdb::default_input_method(tag);
        inputs = winlangdb::transform_input_methods(inputs, tag);
        input::install_layout(inputs)
            .or_else(|_| Err("failed to enable layout".to_owned()))
    }).collect();

    let errors: Vec<&Result<(), String>> =
        results.iter().filter(|x| x.is_err()).collect();

    if errors.len() > 0 {
        return Err("Enabling some layouts caused errors, which were ignored.".to_owned())
    }

    Ok(())
}

pub fn query_language(tag: &str) -> String {
    let id = winnls::resolve_locale_name(tag)
        .unwrap_or(tag.to_owned());

    match winlangdb::get_language_names(&id) {
        None => format!("{}: Unsupported tag.", &id),
        Some(data) => {
            format!("\
Tag: {}
Name: {}
English Name: {}
Native Name: {}
Script: {}", id, data.name, data.english_name, data.localised_name, data.script_name)
        }
    }
}

pub fn enabled_languages() -> Result<Vec<String>, io::Error> {
    winlangdb::ensure_language_profile_exists()?;
    bcp47langs::get_user_languages()
}

// TODO: reimplement support for adding native language name, optionally
pub fn enable_language(tag: &str) -> Result<(), io::Error> {
    let mut langs = enabled_languages()?;
    
    let lang = tag.to_owned();

    if langs.contains(&lang) {
        return Ok(());
    }
    
    langs.push(lang);

    set_user_languages(&langs).unwrap();
    //    .or_else(|_| Err("Error while setting languages.".to_owned()))
    Ok(())
}

// pub fn system_locales() -> Vec<String> {
//     unsafe extern "system" fn callback(locale: LPWSTR, _: DWORD, l_param: LPARAM) -> i32 {
//         let s = lpwstr_to_string(locale);
//         let vec = l_param as *mut Vec<String>;
//         (*vec).push(s);
//         1
//     }
//     let raw_vec = Box::into_raw(Box::new(vec![]));
//     unsafe {
//         winapi::um::winnls::EnumSystemLocalesEx(Some(callback), 0, raw_vec as LPARAM, null_mut());
//         *Box::from_raw(raw_vec)
//     }
// }

// fn base_regkey(is_all_users: bool) -> RegKey {
//     match is_all_users {
//         true => {
//             RegKey::predef(HKEY_USERS)
//                 .open_subkey_with_flags(r".DEFAULT", KEY_READ | KEY_WRITE)
//                 .unwrap()
//         },
//         false => RegKey::predef(HKEY_CURRENT_USER)
//     }
// }

// #[test]
// fn test_sub_id() {
//     println!("sub_id: {:08x}", next_substitute_id(0xabcd));
//     println!("sub_id: {:08x}", next_substitute_id(0x0c09));
// }

// #[test]
// fn test_it_doth_work() {
//     let v = LanguageRegKey::next_transient_lang_id();

//     println!("Transient id: {:04x}", v);
// }

// #[test]
// fn test_lcid_from_bcp47() {
//     assert_eq!(bcp47langs::lcid_from_bcp47("en-AU"), Some(0x0c09), "en-AU");
//     assert_eq!(bcp47langs::lcid_from_bcp47("vro-Latn"), Some(0x2000), "vro-Latn");
//     assert_eq!(bcp47langs::lcid_from_bcp47("sjd-Cyrl"), Some(0x1000), "sjd-Cyrl");
// }

// #[test]
// fn test_default_input() {
//     assert_eq!(winlangdb::default_input_method("en-AU"), InputList("0C09:00000409".to_owned()))
// }

// #[test]
// fn test_transform_input() {
//     assert_eq!(
//         winlangdb::transform_input_methods(InputList("0C09:00000409".to_owned()), "en-AU"),
//         InputList("0C09:00000409".to_owned())
//     );
// }