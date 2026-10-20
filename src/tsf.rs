//! Language profiles of the Divvun keyboard text service (kbdgen's
//! `docs/spec/tsf.md`, "Registration"). The text service's own installer
//! registers its CLSID; kbdi registers, enables and removes one profile per
//! keyboard layout.
use crate::platform::profile_mgr::ProfileMgr;
use std::{fmt, io, path::PathBuf};
use windows_registry::LOCAL_MACHINE;
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
    System::Registry::{KEY_WOW64_32KEY, KEY_WOW64_64KEY, RegDeleteKeyExW},
};

/// The text service's CLSID (`{5E668C8A-2FB8-41D2-90B1-9C132653FA9D}`), or the
/// GUID `KBDI_TSF_CLSID` held at build time, so a test build of kbdi can drive
/// a test registration of the text service beside an installed one.
// [spec:kbdgen:def:tsf.register.ids]
pub const CLSID: u128 = match option_env!("KBDI_TSF_CLSID") {
    Some(text) => match parse_guid(text) {
        Some(clsid) => clsid,
        None => panic!("KBDI_TSF_CLSID is not a GUID"),
    },
    None => 0x5E66_8C8A_2FB8_41D2_90B1_9C13_2653_FA9D,
};

const fn hex(byte: u8) -> Option<u128> {
    match byte {
        b'0'..=b'9' => Some((byte - b'0') as u128),
        b'a'..=b'f' => Some((byte - b'a' + 10) as u128),
        b'A'..=b'F' => Some((byte - b'A' + 10) as u128),
        _ => None,
    }
}

/// Parses `XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX` in either case, with or
/// without either brace: older installers wrote `Layout Product Code`
/// without its closing brace.
pub const fn parse_guid(text: &str) -> Option<u128> {
    let mut bytes = text.as_bytes();
    if let [b'{', rest @ ..] = bytes {
        bytes = rest;
    }
    if let [rest @ .., b'}'] = bytes {
        bytes = rest;
    }
    if bytes.len() != 36 {
        return None;
    }
    let mut value: u128 = 0;
    let mut position = 0;
    while let [byte, rest @ ..] = bytes {
        if matches!(position, 8 | 13 | 18 | 23) {
            if *byte != b'-' {
                return None;
            }
        } else {
            match hex(*byte) {
                Some(digit) => value = (value << 4) | digit,
                None => return None,
            }
        }
        position += 1;
        bytes = rest;
    }
    Some(value)
}

pub fn braced(guid: u128) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:04X}-{:012X}}}",
        guid >> 96,
        (guid >> 80) & 0xFFFF,
        (guid >> 64) & 0xFFFF,
        (guid >> 48) & 0xFFFF,
        guid & 0xFFFF_FFFF_FFFF
    )
}

/// A text service profile in an input-method list: `LLLL:{CLSID}{GUID}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TipInput {
    pub lang_id: u16,
    pub clsid: u128,
    pub profile: u128,
}

impl TipInput {
    /// The keyboard's profile under the text service's CLSID.
    pub fn new(lang_id: u16, profile: u128) -> Self {
        Self {
            lang_id,
            clsid: CLSID,
            profile,
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let (lang, rest) = text.split_once(':')?;
        let lang = lang
            .strip_prefix("0x")
            .or_else(|| lang.strip_prefix("0X"))
            .unwrap_or(lang);
        if lang.len() != 4 || rest.len() != 76 || !rest.is_char_boundary(38) {
            return None;
        }
        let (clsid, profile) = rest.split_at(38);
        if !clsid.starts_with('{') || !profile.ends_with('}') {
            return None;
        }
        Some(Self {
            lang_id: u16::from_str_radix(lang, 16).ok()?,
            clsid: parse_guid(clsid)?,
            profile: parse_guid(profile)?,
        })
    }

    /// Whether this is `profile` under the text service's CLSID, in any language.
    pub fn is_profile(&self, profile: u128) -> bool {
        self.clsid == CLSID && self.profile == profile
    }
}

impl fmt::Display for TipInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04X}:{}{}",
            self.lang_id,
            braced(self.clsid),
            braced(self.profile)
        )
    }
}

/// Whether two input-method strings name the same layout or profile. Windows
/// lists them with or without `0x` and in either case.
pub fn same_input(a: &str, b: &str) -> bool {
    if let (Some(a), Some(b)) = (TipInput::parse(a), TipInput::parse(b)) {
        return a == b;
    }
    fn plain(text: &str) -> String {
        text.to_ascii_lowercase().replace("0x", "")
    }
    plain(a) == plain(b)
}

fn native_key(path: &str) -> windows_registry::Result<windows_registry::Key> {
    // The 64-bit view, also from the x86 kbdi on 64-bit Windows: it holds
    // the registration every native process loads.
    LOCAL_MACHINE.options().read().wow64_64().open(path)
}

/// The text service's key under `CTF\TIP` in `HKLM`.
fn tip_path() -> String {
    format!(r"SOFTWARE\Microsoft\CTF\TIP\{}", braced(CLSID))
}

/// The LANGID that a key under `LanguageProfile` is named for: TSF names
/// them `0x00002400`.
fn language_key_lang_id(name: &str) -> Option<u16> {
    let digits = name
        .strip_prefix("0x")
        .or_else(|| name.strip_prefix("0X"))
        .unwrap_or(name);
    u16::try_from(u32::from_str_radix(digits, 16).ok()?).ok()
}

/// Whether the text service is installed: its CLSID names an existing DLL.
// [spec:kbdgen:req:tsf.register.profile]
pub fn available() -> bool {
    let path = format!(r"SOFTWARE\Classes\CLSID\{}\InprocServer32", braced(CLSID));
    match native_key(&path).and_then(|key| key.get_string("")) {
        Ok(server) if !server.is_empty() && std::path::Path::new(&server).is_file() => true,
        Ok(server) => {
            log::info!("Text service server {server:?} is missing");
            false
        }
        Err(_) => {
            log::info!("Text service {} is not registered", braced(CLSID));
            false
        }
    }
}

/// The LANGIDs under which `profile` is registered, read from the profiles
/// TSF wrote. kbdi registers and unregisters profiles only through
/// `ProfileMgr`; see `remove_emptied_languages` for what it deletes itself.
pub fn registered_lang_ids(profile: u128) -> io::Result<Vec<u16>> {
    let path = format!(r"{}\LanguageProfile", tip_path());
    let Ok(languages) = native_key(&path) else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for language in crate::registry_snapshot::keys(&languages)? {
        let Some(lang_id) = language_key_lang_id(&language) else {
            continue;
        };
        let Ok(profiles) = native_key(&format!(r"{path}\{language}")) else {
            continue;
        };
        if crate::registry_snapshot::keys(&profiles)?
            .iter()
            .any(|name| parse_guid(name) == Some(profile))
        {
            result.push(lang_id);
        }
    }
    Ok(result)
}

fn system_directory() -> io::Result<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    let mut buffer = vec![0u16; 260];
    // SAFETY: writable buffer of the declared length.
    let length = unsafe {
        windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
    } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(io::Error::last_os_error());
    }
    Ok(OsString::from_wide(&buffer[..length]).into())
}

/// Registers `profile` under `lang_id` unless TSF already holds it there.
/// Registering under one LANGID never removes it from another, so profiles
/// other users enabled under their transient LANGIDs stay.
// [spec:kbdgen:req:tsf.register.profile]
// [spec:kbdgen:req:tsf.register.langid]
pub fn ensure_profile(
    lang_id: u16,
    profile: u128,
    name: &str,
    layout_file: &str,
) -> io::Result<()> {
    if registered_lang_ids(profile)?.contains(&lang_id) {
        log::debug!(
            "Profile {} already registered under {lang_id:04X}",
            braced(profile)
        );
        return Ok(());
    }
    // The icon is the layout DLL's in the native system directory, which
    // 32-bit processes reach through redirection to its wow64 variant.
    let icon = system_directory()?.join(layout_file);
    log::info!(
        "Registering profile {} under {lang_id:04X} with icon {}",
        braced(profile),
        icon.display()
    );
    ProfileMgr::new()?.register(CLSID, lang_id, profile, name, &icon.to_string_lossy())?;
    if !registered_lang_ids(profile)?.contains(&lang_id) {
        return Err(io::Error::other(format!(
            "TSF did not register profile {} under {lang_id:04X}",
            braced(profile)
        )));
    }
    Ok(())
}

/// Unregisters `profile` under every LANGID TSF holds it under, then
/// deletes what that left empty.
// [spec:kbdgen:req:tsf.register.uninstall+2]
pub fn unregister_profile(profile: u128) -> io::Result<()> {
    let lang_ids = registered_lang_ids(profile)?;
    if lang_ids.is_empty() {
        return Ok(());
    }
    let manager = ProfileMgr::new()?;
    for &lang_id in &lang_ids {
        log::info!(
            "Unregistering profile {} under {lang_id:04X}",
            braced(profile)
        );
        manager.unregister(CLSID, lang_id, profile)?;
    }
    let left = registered_lang_ids(profile)?;
    if !left.is_empty() {
        return Err(io::Error::other(format!(
            "TSF kept profile {} under {left:04X?}",
            braced(profile)
        )));
    }
    remove_emptied_languages(&lang_ids)
}

/// The views of `HKLM\SOFTWARE` that hold `CTF\TIP`: the 64-bit view and
/// `WOW6432Node`, which 32-bit processes read. On 32-bit Windows both name
/// the one view, and on Windows 10 x64 both views share `CTF\TIP`.
const VIEWS: [(u32, &str); 2] = [(KEY_WOW64_64KEY, "64-bit"), (KEY_WOW64_32KEY, "32-bit")];

/// `UnregisterProfile` leaves the key of the profile's LANGID under
/// `LanguageProfile` behind, without values or subkeys. This deletes the
/// keys of `lang_ids` that are so empty, then `LanguageProfile` itself if
/// that left it empty, in both views. A key with any value or subkey stays,
/// and so do the text service's other keys, such as `Category`, which its
/// own registration writes. These deletions are the only `CTF\TIP` writes
/// kbdi makes itself.
// [spec:kbdgen:req:tsf.register.uninstall+2]
fn remove_emptied_languages(lang_ids: &[u16]) -> io::Result<()> {
    let tip = tip_path();
    for (view, name) in VIEWS {
        let open = |path: &str| LOCAL_MACHINE.options().read().access(view).open(path);
        if let Ok(languages) = open(&format!(r"{tip}\LanguageProfile")) {
            for language in crate::registry_snapshot::keys(&languages)? {
                if language_key_lang_id(&language).is_some_and(|id| lang_ids.contains(&id)) {
                    remove_if_empty(&languages, &language, view, name)?;
                }
            }
        }
        if let Ok(parent) = open(&tip) {
            remove_if_empty(&parent, "LanguageProfile", view, name)?;
        }
    }
    Ok(())
}

/// Deletes `parent\name` in `view` if it has neither values nor subkeys.
fn remove_if_empty(
    parent: &windows_registry::Key,
    name: &str,
    view: u32,
    view_name: &str,
) -> io::Result<()> {
    let Ok(key) = parent.options().read().access(view).open(name) else {
        return Ok(());
    };
    if !crate::registry_snapshot::keys(&key)?.is_empty()
        || !crate::registry_snapshot::values(&key)?.is_empty()
    {
        return Ok(());
    }
    drop(key);
    let wide = crate::winrust::to_wide_string(name);
    // SAFETY: an open key handle and a NUL-terminated name. RegDeleteKeyExW
    // refuses keys that have subkeys.
    let status = unsafe { RegDeleteKeyExW(parent.as_raw(), wide.as_ptr(), view, 0) };
    match status {
        ERROR_SUCCESS => {
            log::info!("Deleted the emptied {name} key of the text service ({view_name} view)");
            Ok(())
        }
        // Deleted since it was opened, through the other view or by another
        // process.
        ERROR_FILE_NOT_FOUND => Ok(()),
        status => Err(io::Error::from_raw_os_error(status as i32)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: u128 = 0x94C7_1262_EE9D_489B_926C_1593_8155_8D90;

    #[test]
    fn tip_strings_round_trip_in_either_form() {
        let input = TipInput::new(0x2000, PROFILE);
        let text = input.to_string();
        assert_eq!(
            text,
            format!(
                "2000:{}{{94C71262-EE9D-489B-926C-159381558D90}}",
                braced(CLSID)
            )
        );
        assert_eq!(TipInput::parse(&text), Some(input));
        assert_eq!(
            TipInput::parse(&format!("0x{}", text.to_ascii_lowercase())),
            Some(input)
        );
        for bad in [
            "0409:A0000409",
            "2000:{guid}{guid}",
            &text[..text.len() - 1],
            &text.replacen(':', "/", 1),
            &format!("{text}x"),
        ] {
            assert_eq!(TipInput::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn inputs_compare_by_value() {
        let text = TipInput::new(0x0409, PROFILE).to_string();
        assert!(same_input(&text, &format!("0x{}", text.to_lowercase())));
        assert!(!same_input(
            &text,
            &TipInput::new(0x0410, PROFILE).to_string()
        ));
        assert!(same_input("0409:A0000409", "0x0409:a0000409"));
        assert!(!same_input("0409:A0000409", "0409:A0010409"));
        assert!(!same_input("0409:A0000409", &text));
    }

    #[test]
    fn language_keys_name_their_lang_id() {
        assert_eq!(language_key_lang_id("0x00002400"), Some(0x2400));
        assert_eq!(language_key_lang_id("0X0000043B"), Some(0x043B));
        assert_eq!(language_key_lang_id("0c3b"), Some(0x0C3B));
        for bad in ["", "0x", "0x00012400", "Category", "{5E668C8A}"] {
            assert_eq!(language_key_lang_id(bad), None, "{bad}");
        }
    }

    #[test]
    fn product_codes_parse_with_or_without_braces() {
        let braced = braced(PROFILE);
        assert_eq!(parse_guid(&braced), Some(PROFILE));
        assert_eq!(parse_guid(&braced[..braced.len() - 1]), Some(PROFILE));
        assert_eq!(parse_guid(&braced.to_lowercase()), Some(PROFILE));
        assert_eq!(parse_guid("{94C71262-EE9D-489B-926C}"), None);
        assert!(TipInput::new(0, PROFILE).is_profile(PROFILE));
        assert!(!TipInput::new(0, PROFILE).is_profile(PROFILE ^ 1));
    }
}
