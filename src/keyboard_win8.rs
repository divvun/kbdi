use crate::keyboard::{Error, KeyboardRegKey};
use crate::platform::*;
use crate::registry_snapshot;
use crate::types::*;
use std::convert::TryFrom;
use std::io;
use windows_registry::{CURRENT_USER, Key as RegKey};

pub fn enable(tag: &str, product_code: &str, _lang_name: Option<&str>) -> Result<(), Error> {
    crate::win8::validate_language_tag(tag)?;
    let record = KeyboardRegKey::find_by_product_code(product_code).ok_or(Error::NotFound)?;
    let original_languages = crate::enabled_languages()?;
    let already_enabled = original_languages
        .iter()
        .any(|value| value.eq_ignore_ascii_case(tag));
    let original_layout = winuser::current_keyboard();

    let result = (|| -> io::Result<()> {
        crate::enable_language(tag)?;
        // Custom language IDs are allocated when the profile is enabled.
        let lcid = bcp47langs::lcid_from_bcp47(tag).ok_or_else(|| {
            io::Error::other(format!("Windows did not allocate a language ID for {tag}"))
        })?;
        let tip = InputList::try_from(format!("{lcid:04X}:{}", record.regkey_id()))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid keyboard input ID"))?;
        // Add only the requested keyboard. Do not rewrite unrelated languages,
        // IMEs, preload order, or substitutions.
        input::install_layout(tip, 0)
    })();
    if let Err(error) = result {
        if !already_enabled {
            winlangdb::set_user_languages(&original_languages).map_err(|rollback| {
                io::Error::other(format!(
                    "{error}; restoring language profiles failed: {rollback}"
                ))
            })?;
        }
        winuser::set_active_keyboard(original_layout);
        return Err(error.into());
    }
    winuser::set_active_keyboard(original_layout);
    coreglobconfig::sync_language_data();
    Ok(())
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

pub fn remove_invalid_kbids() -> io::Result<()> {
    let installed: Vec<String> = KeyboardRegKey::installed()
        .iter()
        .map(|key| key.regkey_id().to_owned())
        .collect();
    let stale = stale_keyboard_inputs(user_input_methods()?, &installed);
    remove_selected_inputs(stale, input::install_layout)
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
