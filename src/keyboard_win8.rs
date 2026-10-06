use crate::keyboard::{Error, KeyboardRegKey};
use crate::platform::*;
use crate::registry_snapshot;
use crate::tsf::{self, TipInput};
use crate::types::*;
use std::convert::TryFrom;
use std::io;
use windows_registry::{CURRENT_USER, Key as RegKey, USERS};

/// Adds the keyboard to the user's input list: the text service's profile
/// when the text service is installed, otherwise the layout, never both.
// [spec:kbdgen:req:tsf.register.enable]
pub fn enable(tag: &str, product_code: &str, _lang_name: Option<&str>) -> Result<(), Error> {
    crate::win8::validate_language_tag(tag)?;
    let record = KeyboardRegKey::find_by_product_code(product_code).ok_or(Error::NotFound)?;
    let original_languages = crate::enabled_languages()?;
    let already_enabled = original_languages
        .iter()
        .any(|value| value.eq_ignore_ascii_case(tag));
    let _original_layout = text_session::ActiveKeyboard::capture();

    let result = (|| -> io::Result<()> {
        crate::enable_language(tag)?;
        // Custom language IDs are allocated when the profile is enabled.
        let lcid = bcp47langs::lcid_from_bcp47(tag).ok_or_else(|| {
            io::Error::other(format!("Windows did not allocate a language ID for {tag}"))
        })?;
        let lang_id = lcid as u16;
        let layout = format!("{lang_id:04X}:{}", record.regkey_id().to_ascii_uppercase());
        InputList::try_from(layout.clone())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid keyboard input ID"))?;
        let profile = record
            .product_code()
            .and_then(|code| tsf::parse_guid(&code));
        let expected = match profile.and_then(|profile| register_profile(&record, lang_id, profile))
        {
            Some(tip) => tip.to_string(),
            None => layout,
        };
        // Add only the requested keyboard. Do not rewrite unrelated languages,
        // IMEs, preload order, or substitutions.
        input::install(&expected, 0)?;
        if !user_input_methods()?
            .iter()
            .any(|id| tsf::same_input(id, &expected))
        {
            return Err(io::Error::other(format!(
                "Windows did not save keyboard input {expected}"
            )));
        }
        // Then remove the other form, which an earlier install or a text
        // service installed or removed since may have left enabled.
        if TipInput::parse(&expected).is_some() {
            disable_keyboard(record.regkey_id())?;
        } else if let Some(profile) = profile {
            disable_profile(profile)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        if !already_enabled {
            winlangdb::set_user_languages(&original_languages).map_err(|rollback| {
                io::Error::other(format!(
                    "{error}; restoring language profiles failed: {rollback}"
                ))
            })?;
        }
        return Err(error.into());
    }
    coreglobconfig::sync_language_data();
    Ok(())
}

/// The keyboard's profile under `lang_id`, registered now if needed, or
/// `None` to fall back to the layout because the text service is absent or
/// refused the profile.
// [spec:kbdgen:req:tsf.register.langid]
fn register_profile(record: &KeyboardRegKey, lang_id: u16, profile: u128) -> Option<TipInput> {
    if !tsf::available() {
        return None;
    }
    let file = record.layout_file()?;
    let name = record.layout_name().unwrap_or_default();
    match tsf::ensure_profile(lang_id, profile, &name, &file) {
        Ok(()) => Some(TipInput::new(lang_id, profile)),
        Err(error) => {
            log::warn!("Text service profile unavailable, enabling the layout: {error}");
            None
        }
    }
}

/// Registers the profile of a newly installed layout whose tag has a
/// Windows locale, under the LANGID of its KLID. Profiles of tags without
/// one are registered by `enable`, under the user's transient LANGID.
// [spec:kbdgen:req:tsf.register.profile]
pub fn register_installed_profile(tag: &str, product_code: &str) {
    let Some(record) = KeyboardRegKey::find_by_product_code(product_code) else {
        return;
    };
    let lcid = match winnls::locale_name_to_lcid(tag) {
        Ok(lcid) if lcid != 0x1000 => lcid,
        _ => return,
    };
    if let Some(profile) = record
        .product_code()
        .and_then(|code| tsf::parse_guid(&code))
    {
        register_profile(&record, lcid as u16, profile);
    }
}

/// Offers the keyboard on the welcome screen as its layout, not the text
/// service's profile, so sign-in never depends on the text service.
// [spec:kbdgen:req:tsf.register.welcome]
pub fn enable_default_user(product_code: &str) -> Result<(), Error> {
    let record = KeyboardRegKey::find_by_product_code(product_code).ok_or(Error::NotFound)?;
    let layout = default_user_layout(record.regkey_id())?;
    input::install(&layout, input::ILOT_DEFUSER4)?;
    Ok(())
}

/// Takes the layout off the welcome screen if `enable_default_user` put it
/// there, before its KLID is removed.
// [spec:kbdgen:req:tsf.register.welcome]
pub fn disable_default_user(klid: &str) -> io::Result<()> {
    if !default_user_has(klid)? {
        return Ok(());
    }
    input::install(
        &default_user_layout(klid)?,
        input::ILOT_DEFUSER4 | input::ILOT_UNINSTALL,
    )?;
    if default_user_has(klid)? {
        return Err(io::Error::other(format!(
            "Windows kept keyboard {klid} on the welcome screen"
        )));
    }
    Ok(())
}

/// Whether `.DEFAULT` preloads the KLID, directly or through a substitute.
fn default_user_has(klid: &str) -> io::Result<bool> {
    let mut found = false;
    for name in ["Preload", "Substitutes"] {
        let Ok(key) = USERS.open(format!(r".DEFAULT\Keyboard Layout\{name}")) else {
            continue;
        };
        found |= registry_snapshot::values(&key)?
            .into_iter()
            .any(|(_, value)| {
                String::try_from(value).is_ok_and(|value| value.eq_ignore_ascii_case(klid))
            });
    }
    Ok(found)
}

/// The welcome screen's input for a KLID: under the LANGID of the KLID's
/// low word, which is the language `install` created it for.
fn default_user_layout(klid: &str) -> io::Result<String> {
    let input = InputListItem::try_from(format!("{}:{klid}", klid.get(4..).unwrap_or("")).as_str())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid keyboard input ID"))?;
    Ok(format!("{}:{}", input.lcid(), input.kbid()))
}

fn selected_keyboard_inputs(
    enabled: impl IntoIterator<Item = String>,
    select: impl Fn(&InputListItem) -> bool,
) -> InputList {
    enabled
        .into_iter()
        // TSF text service IDs and malformed records are not keyboard IDs.
        // Leave them untouched instead of clearing and rebuilding the full list.
        .filter_map(|value| InputListItem::try_from(value.as_str()).ok())
        .filter(select)
        .collect::<Vec<_>>()
        .into()
}

fn stale_keyboard_inputs(enabled: Vec<String>, installed: &[String]) -> InputList {
    selected_keyboard_inputs(enabled, |item| {
        let id = item.kbid();
        id.starts_with('A')
            && !installed
                .iter()
                .any(|value| value.eq_ignore_ascii_case(&id))
    })
}

fn remove_selected_inputs(
    inputs: InputList,
    mut remove: impl FnMut(InputList, i32) -> io::Result<()>,
) -> io::Result<()> {
    if !inputs.inner().is_empty() {
        remove(inputs, input::ILOT_UNINSTALL)?;
    }
    Ok(())
}

fn user_input_methods() -> io::Result<Vec<String>> {
    Ok(crate::enabled_keyboards()?
        .into_iter()
        .flat_map(|(_, methods)| methods)
        .collect())
}

pub fn disable_keyboard(kbid: &str) -> io::Result<()> {
    let selected = selected_keyboard_inputs(user_input_methods()?, |item| {
        item.kbid().eq_ignore_ascii_case(kbid)
    });
    remove_selected_inputs(selected, input::install_layout)
}

/// The user's inputs of the text service's profiles that `select` picks.
fn selected_profile_inputs(
    enabled: impl IntoIterator<Item = String>,
    select: impl Fn(&TipInput) -> bool,
) -> Vec<TipInput> {
    enabled
        .into_iter()
        .filter_map(|value| TipInput::parse(&value))
        .filter(|tip| tip.clsid == tsf::CLSID && select(tip))
        .collect()
}

fn remove_profile_inputs(
    inputs: Vec<TipInput>,
    remove: impl FnOnce(&str, i32) -> io::Result<()>,
) -> io::Result<()> {
    if inputs.is_empty() {
        return Ok(());
    }
    let list = inputs
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(";");
    remove(&list, input::ILOT_UNINSTALL)
}

/// Removes the profile from the current user's inputs, in every language.
// [spec:kbdgen:req:tsf.register.uninstall]
pub fn disable_profile(profile: u128) -> io::Result<()> {
    let selected = selected_profile_inputs(user_input_methods()?, |tip| tip.profile == profile);
    remove_profile_inputs(selected, input::install)?;
    // Windows ignores the removal of a profile whose text service is no
    // longer registered. The text service's uninstaller runs only after the
    // last profile is gone, so this is a broken install, not an error here.
    let kept = selected_profile_inputs(user_input_methods()?, |tip| tip.profile == profile);
    if !kept.is_empty() {
        log::warn!("Windows kept text service inputs {kept:?}");
    }
    Ok(())
}

pub fn remove_invalid_kbids() -> io::Result<()> {
    let installed = KeyboardRegKey::installed();
    let klids: Vec<String> = installed
        .iter()
        .map(|key| key.regkey_id().to_owned())
        .collect();
    let stale = stale_keyboard_inputs(user_input_methods()?, &klids);
    remove_selected_inputs(stale, input::install_layout)?;
    let profiles: Vec<u128> = installed
        .iter()
        .filter_map(|key| key.product_code().and_then(|code| tsf::parse_guid(&code)))
        .collect();
    let stale = selected_profile_inputs(user_input_methods()?, |tip| {
        !profiles.contains(&tip.profile)
    });
    remove_profile_inputs(stale, input::install)
}

pub fn regenerate_registry() {
    let user_profile_key = CURRENT_USER
        .open(r"Control Panel\International\User Profile")
        .unwrap();
    let substitutes_key = CURRENT_USER
        .options()
        .read()
        .write()
        .open(r"Keyboard Layout\Substitutes")
        .unwrap();
    let preload_key = CURRENT_USER
        .options()
        .read()
        .write()
        .open(r"Keyboard Layout\Preload")
        .unwrap();

    regenerate_given_registry(user_profile_key, substitutes_key, preload_key);
}

fn regenerate_given_registry(
    user_profile_key: RegKey,
    substitutes_key: RegKey,
    preload_key: RegKey,
) {
    if native::is_local_system().expect("read process identity") {
        log::debug!("Not refreshing because we're running at NT Authority/System");
        return;
    }

    log::debug!("regenerate_given_registry");
    let lang_keys: Vec<_> = registry_snapshot::keys(&user_profile_key)
        .unwrap()
        .into_iter()
        .map(|name| user_profile_key.open(name).unwrap())
        .collect();

    log::trace!("Lang keys: {:?}", lang_keys);

    // Get known keyboard ids from Control Panel configured language list
    let mut keyboard_ids: Vec<_> = lang_keys
        .iter()
        .flat_map(|k| registry_snapshot::values(k).unwrap())
        .map(|(name, _)| name)
        .filter(|n| n.contains(":"))
        .map(|v| InputListItem::try_from(&*v))
        // .map(|n| n.split(":").last().unwrap().to_string())
        .filter_map(Result::ok)
        .collect();

    keyboard_ids.sort_by(|a, b| a.tip_id.cmp(&b.tip_id));
    keyboard_ids.sort_by(|a, b| a.lang_id.cmp(&b.lang_id));

    log::trace!("Keyboard IDs: {:?}", &keyboard_ids);

    // Get all substitutes into a list
    let subs = registry_snapshot::values(&substitutes_key)
        .unwrap()
        .into_iter()
        .map(|(name, value)| (name, String::try_from(value).expect("string substitute")))
        .collect::<Vec<_>>();

    log::trace!("Substitutions: {:?}", &subs);

    // Clean up all invalid substitutions
    for (value_id, kbd_id) in subs.iter() {
        if keyboard_ids
            .iter()
            .find(|x| &format!("{:08x}", x.tip_id) == kbd_id)
            .is_none()
        {
            log::debug!("Deleting substitute: {:?}", value_id);
            substitutes_key.remove_value(value_id).unwrap();
        }
    }

    // Delete all preload values
    for (name, _) in registry_snapshot::values(&preload_key).unwrap() {
        preload_key.remove_value(name).unwrap();
    }

    log::trace!("Cleared all preload keys");

    // Check if substitutes contains lang_id
    for (i, item) in keyboard_ids.iter().enumerate() {
        let lcid = format!("{:08x}", item.lang_id);
        let tip = format!("{:08x}", item.tip_id);

        let value = if let Some(sub) = subs
            .iter()
            .filter(|sub| sub.1 == tip && sub.0[4..] == lcid[4..])
            .nth(0)
        {
            log::trace!("{}: Adding substitute lcid: {}", i + 1, &sub.0);
            sub.0.clone()
        } else {
            log::trace!("{}: Adding TIP: {}", i + 1, &tip);
            tip
        };

        preload_key.set_string((i + 1).to_string(), &value).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_removes_only_missing_custom_keyboards() {
        let enabled = [
            "0409:00000409",
            "0419:00000419",
            "2400:A0002400",
            "2400:A0012400",
            "0411:{tip-guid}{profile-guid}",
            "malformed",
        ];
        let installed = vec!["a0002400".to_owned()];
        let stale = stale_keyboard_inputs(
            enabled.iter().map(|value| value.to_string()).collect(),
            &installed,
        );
        let mut called = false;
        remove_selected_inputs(stale, |inputs, flags| {
            called = true;
            assert_eq!(flags, 1);
            assert_eq!(String::from(inputs), "0x2400:A0012400");
            Ok(())
        })
        .unwrap();
        assert!(called);
    }

    #[test]
    fn cleanup_noop_does_not_touch_input_state_and_failure_is_propagated() {
        let valid = vec!["0409:00000409".to_owned(), "2400:A0002400".to_owned()];
        let selected = stale_keyboard_inputs(valid, &["a0002400".to_owned()]);
        remove_selected_inputs(selected, |_, _| panic!("no input changes expected")).unwrap();
        let stale = stale_keyboard_inputs(vec!["2400:A0012400".to_owned()], &[]);
        let error =
            remove_selected_inputs(stale, |_, _| Err(io::ErrorKind::PermissionDenied.into()))
                .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn profile_selection_picks_only_this_text_service_profile() {
        let ours = TipInput::new(0x2000, 7);
        let other_language = TipInput::new(0x0409, 7);
        let other_profile = TipInput::new(0x2000, 8);
        let other_service = TipInput {
            clsid: tsf::CLSID ^ 1,
            ..ours
        };
        let inputs = [
            ours.to_string(),
            "2000:A0002000".to_owned(),
            other_language.to_string().to_ascii_lowercase(),
            other_profile.to_string(),
            other_service.to_string(),
        ];
        let selected = selected_profile_inputs(inputs, |tip| tip.profile == 7);
        assert_eq!(selected, vec![ours, other_language]);
        let mut called = false;
        remove_profile_inputs(selected, |list, flags| {
            called = true;
            assert_eq!(flags, input::ILOT_UNINSTALL);
            assert_eq!(list, format!("{ours};{other_language}"));
            Ok(())
        })
        .unwrap();
        assert!(called);
        remove_profile_inputs(Vec::new(), |_, _| panic!("no input changes expected")).unwrap();
    }

    #[test]
    fn welcome_screen_layout_uses_the_klid_language() {
        assert_eq!(default_user_layout("a0002000").unwrap(), "2000:A0002000");
        assert_eq!(default_user_layout("A001043b").unwrap(), "043B:A001043B");
        assert!(default_user_layout("a00").is_err());
    }

    #[test]
    fn uninstall_selection_preserves_other_keyboards_and_text_services() {
        let inputs = [
            "0409:A0002400",
            "2400:A0002400",
            "2400:A0012400",
            "0409:00000409",
            "0411:{tip}{profile}",
        ];
        let selected =
            selected_keyboard_inputs(inputs.iter().map(|value| value.to_string()), |item| {
                item.kbid().eq_ignore_ascii_case("a0002400")
            });
        assert_eq!(String::from(selected), "0x0409:A0002400;0x2400:A0002400");
    }
}
