use crate::registry_snapshot;
use windows_registry::CURRENT_USER;

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
    enabled_languages()?
        .into_iter()
        .map(|lang| {
            let imes = bcp47langs::get_user_language_input_methods(&lang)?;
            Ok((lang, imes))
        })
        .collect()
}

pub(crate) fn validate_language_tag(tag: &str) -> io::Result<()> {
    if tag.is_empty() || tag.contains(['\0', ';']) || winlangdb::get_language_names(tag).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported language tag: {tag:?}"),
        ));
    }
    Ok(())
}

// TODO: reimplement support for adding native language name, optionally
pub fn enable_language(tag: &str) -> Result<(), io::Error> {
    validate_language_tag(tag)?;
    log::debug!("enable_languages({:?})", tag);
    let mut langs = enabled_languages()?;
    log::trace!("Enabled languages: {:?}", langs);
    let lang = tag.to_owned();

    if langs.iter().any(|value| value.eq_ignore_ascii_case(&lang)) {
        log::debug!("Lang found in langs, doing nothing.");
        return Ok(());
    }

    langs.push(lang);

    set_user_languages(&langs).map_err(io::Error::other)?;

    // winlangdb::ensure_language_profile_exists()?;
    //    .or_else(|_| Err("Error while setting languages.".to_owned()))
    Ok(())
}

fn set_user_languages(tags: &[String]) -> Result<(), String> {
    log::debug!("set_user_languages({:?})", &tags);
    // Existing language profiles must survive even when name lookup is unavailable.
    // Only newly requested tags are validated by enable_language.
    winlangdb::set_user_languages(tags).map_err(|error| error.to_string())?;

    // Workaround for bug in Windows 10 20H2
    win10_20h2_workaround()?;

    Ok(())
}

fn win10_20h2_workaround() -> Result<(), String> {
    let user_profile_key = CURRENT_USER
        .options()
        .read()
        .write()
        .open(r"Control Panel\International\User Profile")
        .map_err(|error| error.to_string())?;

    for name in registry_snapshot::keys(&user_profile_key).map_err(|e| e.to_string())? {
        let subkey = user_profile_key
            .options()
            .read()
            .write()
            .open(&name)
            .map_err(|e| e.to_string())?;
        if subkey.get_value("FeaturesToInstall").is_err() {
            log::debug!(
                "20H2 Workaround: setting FeaturesToInstall to 0xe3 for {}",
                name
            );
            subkey
                .set_u32("FeaturesToInstall", 0xe3)
                .map_err(|e| format!("{:?}", e))?;
        }
    }

    Ok(())
}

fn disable_empty_languages() -> Result<(), io::Error> {
    let langs = enabled_languages()?;
    let mut filtered_langs = Vec::new();
    for tag in &langs {
        if !bcp47langs::get_user_language_input_methods(tag)?.is_empty() {
            filtered_langs.push(tag.clone());
        }
    }
    // Do not attempt to replace the language list with an empty list.
    if !filtered_langs.is_empty() && filtered_langs != langs {
        set_user_languages(&filtered_langs).map_err(io::Error::other)?;
    }
    Ok(())
}

pub fn clean() -> Result<(), String> {
    crate::keyboard::remove_invalid().map_err(|error| error.to_string())?;
    disable_empty_languages().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_language_validation_is_read_only() {
        let before = enabled_languages().unwrap();
        for tag in ["", "en-US;ru", "en-US\0ru", "rus-Cyrl-NO", "rus-Cyrl-DE"] {
            assert_eq!(
                validate_language_tag(tag).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        assert_eq!(enabled_languages().unwrap(), before);
        validate_language_tag("en-US").unwrap();
    }
}
