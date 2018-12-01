use crate::platform::*;
use std::io;

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

pub fn enabled_languages() -> Result<Vec<String>, io::Error> {
    winlangdb::ensure_language_profile_exists()?;
    bcp47langs::get_user_languages()
}

type LangKeyboards = (String, Vec<String>);

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

fn set_user_languages(tags: &[String]) -> Result<(), String> {
    let valid_tags: Vec<String> =
        tags.iter()
        .flat_map(|t| winlangdb::get_language_names(t))
        .map(|t| t.tag)
        .collect();

    winlangdb::set_user_languages(&valid_tags)
        .or_else(|_| Err("Failed enabling languages".to_owned()))
}

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
    crate::keyboard::remove_invalid();
    disable_empty_languages().unwrap();
    Ok(())
}
