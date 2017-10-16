extern crate winapi;
extern crate winreg;
extern crate libloading;
#[macro_use]
extern crate lazy_static;

use std::io;
use winrust::*;
use platform::*;

mod types;
mod winrust;
mod language;
pub mod keyboard;
#[cfg(not(feature = "legacy"))]
mod keyboard_win8;
#[cfg(feature = "legacy")]
mod keyboard_legacy;
pub mod platform;

type LangKeyboards = (String, Vec<String>);

#[cfg(not(feature = "legacy"))]
fn set_user_languages(tags: &[String]) -> Result<(), String> {
    let valid_tags: Vec<String> =
        tags.iter()
        .flat_map(|t| winlangdb::get_language_names(t))
        .map(|t| t.tag)
        .collect();

    winlangdb::set_user_languages(&valid_tags)
        .or_else(|_| Err("Failed enabling languages".to_owned()))
}

#[cfg(not(feature = "legacy"))]
pub fn query_language(tag: &str) -> String {
    let id = winnls::resolve_locale_name(tag)
        .unwrap_or(tag.to_owned());
    
    match winlangdb::get_language_names(&id) {
        None => format!("{}: Unsupported tag.\n", &id),
        Some(v) => {
            let lcid = match bcp47langs::lcid_from_bcp47(&tag) {
                Some(lcid) => format!("LCID:          0x{:08x}", lcid),
                None => format!("LCID:          undefined")
            };
            format!("{}{}", v, lcid)
        }
    }
}

#[cfg(feature = "legacy")]
pub fn query_language(tag: &str) -> String {
    let id = winnls::resolve_locale_name(tag)
        .unwrap_or(tag.to_owned());

    let a = format!("Tag:  {}", id);

    let b = match winnls::locale_name_to_lcid(&id) {
        Ok(lcid) => format!("LCID: 0x{:08x}", lcid),
        Err(_) => format!("LCID: undefined")
    };

    format!("{}\n{}", a, b)
}

#[cfg(not(feature = "legacy"))]
pub fn enabled_languages() -> Result<Vec<String>, io::Error> {
    winlangdb::ensure_language_profile_exists()?;
    bcp47langs::get_user_languages()
}

#[cfg(not(feature = "legacy"))]
pub fn enabled_keyboards() -> Result<Vec<LangKeyboards>, io::Error> {
    let langs = enabled_languages()?;
    Ok(langs.into_iter()
        .map(|lang| {
            let imes = bcp47langs::get_user_language_input_methods(&lang)
                .unwrap();
            (lang, imes)
        })
        .collect())
}

// TODO: reimplement support for adding native language name, optionally
#[cfg(not(feature = "legacy"))]
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

#[cfg(not(feature = "legacy"))]
fn disable_empty_languages() -> Result<(), io::Error> {
    let langs = enabled_languages()?;
    let filtered_langs: Vec<String> = langs.into_iter()
        .filter(|tag| {
            let imes = bcp47langs::get_user_language_input_methods(&tag)
                .unwrap_or(vec![]);
            imes.len() > 0
        })
        .collect();
    
    set_user_languages(&filtered_langs).unwrap();
    Ok(())
        //.or_else(|_| Err("Error while setting languages.".to_owned()))
}

pub fn clean() -> Result<(), String> {
    keyboard::remove_invalid();
    #[cfg(not(feature = "legacy"))]
    disable_empty_languages().unwrap();
    Ok(())
}


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