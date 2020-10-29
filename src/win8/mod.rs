use registry::{Hive, Security};

use crate::platform::*;
use std::io;

pub fn query_language(tag: &str) -> String {
    let id = winnls::resolve_locale_name(tag).unwrap_or(tag.to_owned());

    match winlangdb::get_language_names(&id) {
        None => format!("{}: Unsupported tag.\n", &id),
        Some(v) => {
            let lcid = match bcp47langs::lcid_from_bcp47(&tag) {
                Some(lcid) => format!("LCID:          0x{:08x}", lcid),
                None => format!("LCID:          undefined"),
            };
            format!("{}{}", v, lcid)
        }
    }
}

pub fn enabled_languages() -> Result<Vec<String>, io::Error> {
    // winlangdb::ensure_language_profile_exists()?;
    bcp47langs::get_user_languages()
}

type LangKeyboards = (String, Vec<String>);

pub fn enabled_keyboards() -> Result<Vec<LangKeyboards>, io::Error> {
    log::debug!("enabled_keyboards()");
    let langs = enabled_languages()?;
    Ok(langs
        .into_iter()
        .map(|lang| {
            let imes = bcp47langs::get_user_language_input_methods(&lang).unwrap();
            (lang, imes)
        })
        .collect())
}

// TODO: reimplement support for adding native language name, optionally
pub fn enable_language(tag: &str) -> Result<(), io::Error> {
    log::debug!("enable_languages({:?})", tag);
    let mut langs = enabled_languages()?;
    log::trace!("Enabled languages: {:?}", langs);
    let lang = tag.to_owned();

    if langs.contains(&lang) {
        log::debug!("Lang found in langs, doing nothing.");
        return Ok(());
    }

    langs.push(lang);

    set_user_languages(&langs).unwrap();

    // winlangdb::ensure_language_profile_exists()?;
    //    .or_else(|_| Err("Error while setting languages.".to_owned()))
    Ok(())
}

fn set_user_languages(tags: &[String]) -> Result<(), String> {
    log::debug!("set_user_languages({:?})", &tags);
    let valid_tags: Vec<String> = tags
        .iter()
        .flat_map(|t| winlangdb::get_language_names(t))
        .map(|t| t.tag)
        .collect();

    log::trace!("valid_tags: {:?}", &valid_tags);

    winlangdb::set_user_languages(&valid_tags)
        .or_else(|_| Err("Failed enabling languages".to_owned()))?;

    // Workaround for bug in Windows 10 20H2
    win10_20h2_workaround()?;

    Ok(())
}

fn win10_20h2_workaround() -> Result<(), String> {
    let user_profile_key = Hive::CurrentUser
        .open(
            r"Control Panel\International\User Profile",
            Security::Read | Security::Write,
        )
        .unwrap();

    for subkey in user_profile_key
        .keys()
        .filter_map(Result::ok)
        .map(|x| x.open(Security::Read | Security::Write))
        .filter_map(Result::ok)
    {
        if subkey.value("FeaturesToInstall").is_err() {
            log::debug!("20H2 Workaround: setting FeaturesToInstall to 0xe3 for {}", subkey.to_string());
            subkey
                .set_value("FeaturesToInstall", &registry::Data::U32(0xe3))
                .map_err(|e| format!("{:?}", e))?;
        }
    }

    Ok(())
}

fn disable_empty_languages() -> Result<(), io::Error> {
    let langs = enabled_languages()?;
    let filtered_langs: Vec<String> = langs
        .into_iter()
        .filter(|tag| {
            let imes = bcp47langs::get_user_language_input_methods(&tag).unwrap_or(vec![]);
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
