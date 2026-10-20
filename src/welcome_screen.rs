//! The welcome screen's keyboard layouts, in `HKEY_USERS\.DEFAULT`
//! (kbdgen's `tsf.register.welcome`).
//!
//! The layout goes under the LANGID that the calling user's language list
//! gives the keyboard's tag, under which `keyboard_enable` registered and
//! enabled its profile (`tsf.register.langid`). For a tag without a Windows
//! locale that is a transient LANGID (`0x2000`, `0x2400`, …), which every
//! profile assigns to the languages of its own list. The KLID's LANGID is
//! not it: `install` names such KLIDs for `0x2000` whatever the user's list
//! gives the tag, and the user's list can give `0x2000` to another language,
//! as to English (Sweden) on Windows 10. The welcome screen names transient
//! LANGIDs by `.DEFAULT`'s own language list (`Control Panel\International\
//! User Profile`), which kbdi leaves as it is; on that Windows 10 it gives
//! `0x2000` to English (Sweden) too and the user's other transient LANGIDs
//! to no language. `add` logs what it gives the LANGID. The user's LANGID
//! is also the one Windows itself puts in `.DEFAULT` for the user's
//! languages: in the dummy layouts below, and when it copies the user's
//! settings to the welcome screen, language list included.
//!
//! `InstallLayoutOrTip` with `ILOT_DEFUSER4`, whether it adds or removes a
//! layout, also writes the calling user's hidden dummy layouts into
//! `.DEFAULT`. Windows gives each of a user's languages whose inputs are all
//! text service profiles such a layout: `0000LLLL` in `Preload`, substituted
//! by a real layout and listed under `CTF\HiddenDummyLayouts`. Removing the
//! keyboard's layout leaves them behind. kbdi therefore compares `.DEFAULT`
//! before and after each call. It keeps the dummy layouts enabling added in
//! the KLID's key, and removes them, together with those removing the
//! layout added, when the keyboard leaves the welcome screen, unless another
//! keyboard kbdi put there under the same LANGID stays, which then keeps
//! them. Windows also names a LANGID's layouts anew (`d0012800`,
//! `d0022800`, …) as they come and go; that adds nothing.
use crate::platform::input;
use crate::registry_snapshot;
use crate::types::InputListItem;
use std::convert::TryFrom;
use std::fmt;
use std::io;
use windows_registry::{LOCAL_MACHINE, USERS};

/// The value of a KLID's key that lists what enabling its layout on the
/// welcome screen added to `.DEFAULT` besides the layout. kbdi writes it,
/// empty if nothing was added, whenever it enables the layout there.
const ADDITIONS: &str = "kbdi Welcome Screen Additions";

const KEYBOARD_LAYOUTS: &str = r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts";

/// A key of `.DEFAULT` that `InstallLayoutOrTip` writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum List {
    Preload,
    Substitutes,
    HiddenDummyLayouts,
}

impl List {
    const ALL: [List; 3] = [List::Preload, List::Substitutes, List::HiddenDummyLayouts];

    fn path(self) -> &'static str {
        match self {
            List::Preload => r"Keyboard Layout\Preload",
            List::Substitutes => r"Keyboard Layout\Substitutes",
            List::HiddenDummyLayouts => r"Software\Microsoft\CTF\HiddenDummyLayouts",
        }
    }

    fn from_path(path: &str) -> Option<List> {
        List::ALL.into_iter().find(|list| list.path() == path)
    }
}

/// Something of those keys that a call can add.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Entry {
    Key(List),
    /// A `Preload` value, by its data alone: Windows numbers them by
    /// position and renumbers them.
    Preload(String),
    /// A value of `Substitutes` or `HiddenDummyLayouts`: key, name, data.
    Value(List, String, String),
}

/// The LANGID of a name in those keys, its low word: `0000LLLL` or
/// `d0NNLLLL`.
fn low_word(name: &str) -> Option<u16> {
    (name.len() == 8)
        .then(|| u32::from_str_radix(name, 16).ok())
        .flatten()
        .map(|value| value as u16)
}

impl Entry {
    fn list(&self) -> List {
        match self {
            Entry::Key(list) | Entry::Value(list, _, _) => *list,
            Entry::Preload(_) => List::Preload,
        }
    }

    /// The LANGID of the name of a value; keys have none.
    fn lang_id(&self) -> Option<u16> {
        match self {
            Entry::Key(_) => None,
            Entry::Preload(name) | Entry::Value(_, name, _) => low_word(name),
        }
    }

    fn same(&self, other: &Entry) -> bool {
        match (self, other) {
            (Entry::Key(a), Entry::Key(b)) => a == b,
            (Entry::Preload(a), Entry::Preload(b)) => a.eq_ignore_ascii_case(b),
            (Entry::Value(a, name_a, data_a), Entry::Value(b, name_b, data_b)) => {
                a == b && name_a.eq_ignore_ascii_case(name_b) && data_a.eq_ignore_ascii_case(data_b)
            }
            _ => false,
        }
    }

    /// Whether `before` has this entry, or a value of the same name with
    /// other data: a value Windows changed is not kbdi's to remove.
    fn existed_in(&self, before: &Layouts) -> bool {
        match self {
            Entry::Key(list) => before.keys.contains(list),
            Entry::Preload(data) => before
                .preload
                .iter()
                .any(|(_, old)| old.eq_ignore_ascii_case(data)),
            Entry::Value(list, name, _) => before
                .values
                .iter()
                .any(|(old, old_name, _)| old == list && old_name.eq_ignore_ascii_case(name)),
        }
    }

    /// Parses the forms `Display` writes: `KEY`, `KEY=DATA` for `Preload`
    /// and `KEY\NAME=DATA`.
    fn parse(text: &str) -> Option<Self> {
        let Some((path, data)) = text.rsplit_once('=') else {
            return List::from_path(text).map(Entry::Key);
        };
        if data.is_empty() {
            return None;
        }
        if path == List::Preload.path() {
            return Some(Entry::Preload(data.to_owned()));
        }
        let (key, name) = path.rsplit_once('\\')?;
        match List::from_path(key)? {
            List::Preload => None,
            _ if name.is_empty() => None,
            list => Some(Entry::Value(list, name.to_owned(), data.to_owned())),
        }
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Entry::Key(list) => f.write_str(list.path()),
            Entry::Preload(data) => write!(f, "{}={data}", List::Preload.path()),
            Entry::Value(list, name, data) => write!(f, r"{}\{name}={data}", list.path()),
        }
    }
}

/// Those keys in `.DEFAULT`, and their string values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Layouts {
    keys: Vec<List>,
    /// `Preload`'s numbered values, by number.
    preload: Vec<(u32, String)>,
    /// The values of `Substitutes` and `HiddenDummyLayouts`.
    values: Vec<(List, String, String)>,
}

impl Layouts {
    fn read() -> io::Result<Self> {
        let mut layouts = Layouts::default();
        for list in List::ALL {
            let Ok(key) = USERS.open(format!(r".DEFAULT\{}", list.path())) else {
                continue;
            };
            layouts.keys.push(list);
            for (name, value) in registry_snapshot::values(&key)? {
                let Ok(data) = String::try_from(value) else {
                    continue;
                };
                match list {
                    List::Preload => {
                        if let Ok(number) = name.parse() {
                            layouts.preload.push((number, data));
                        }
                    }
                    _ => layouts.values.push((list, name, data)),
                }
            }
        }
        layouts.preload.sort_by_key(|(number, _)| *number);
        Ok(layouts)
    }

    fn entries(&self) -> impl Iterator<Item = Entry> + '_ {
        let keys = self.keys.iter().copied().map(Entry::Key);
        let preload = self
            .preload
            .iter()
            .map(|(_, data)| Entry::Preload(data.clone()));
        let values = self
            .values
            .iter()
            .map(|(list, name, data)| Entry::Value(*list, name.clone(), data.clone()));
        keys.chain(preload).chain(values)
    }

    /// Whether `Preload` or a substitute names the KLID.
    fn mentions(&self, klid: &str) -> bool {
        self.preload
            .iter()
            .any(|(_, data)| data.eq_ignore_ascii_case(klid))
            || self.values.iter().any(|(list, _, data)| {
                *list == List::Substitutes && data.eq_ignore_ascii_case(klid)
            })
    }

    /// The LANGIDs the KLID is on the welcome screen under, those it was
    /// added under: the low word of a substitute's name (`00002400`, or
    /// `d0012400` when the dummy layout holds that name), and the KLID's
    /// own for a `Preload` value naming the KLID itself.
    fn lang_ids(&self, klid: &str) -> Vec<u16> {
        let direct = self
            .preload
            .iter()
            .filter(|(_, data)| data.eq_ignore_ascii_case(klid))
            .filter_map(|_| low_word(klid));
        let substituted = self
            .values
            .iter()
            .filter(|(list, _, data)| *list == List::Substitutes && data.eq_ignore_ascii_case(klid))
            .filter_map(|(_, name, _)| low_word(name));
        let mut lang_ids = Vec::new();
        for lang_id in direct.chain(substituted) {
            if !lang_ids.contains(&lang_id) {
                lang_ids.push(lang_id);
            }
        }
        lang_ids
    }

    /// The welcome screen's inputs of the KLID, `LLLL:KLID`.
    fn inputs(&self, klid: &str) -> Vec<String> {
        self.lang_ids(klid)
            .into_iter()
            .filter_map(|lang_id| input(lang_id, klid).ok())
            .collect()
    }

    /// What `self` has of hidden dummy layouts that `before` lacks: the
    /// `Preload`, `Substitutes` and `HiddenDummyLayouts` values of the names
    /// `self` lists under `HiddenDummyLayouts`, and the keys holding them.
    /// That is all a call adds besides its layout. The rest it changes are
    /// layouts, its own and others: Windows names a LANGID's layouts anew
    /// (`d0012800`, `d0022800`, …) when one comes or goes.
    fn added_since(&self, before: &Layouts) -> Vec<Entry> {
        let dummy = |name: &str| {
            self.values.iter().any(|(list, hidden, _)| {
                *list == List::HiddenDummyLayouts && hidden.eq_ignore_ascii_case(name)
            })
        };
        self.entries()
            .filter(|entry| match entry {
                Entry::Key(_) => true,
                Entry::Preload(name) | Entry::Value(_, name, _) => dummy(name),
            })
            .filter(|entry| !entry.existed_in(before))
            .collect()
    }

    /// `self` without those of `entries` it still has with the same data.
    /// Keys go only once they hold no values.
    fn without(&self, entries: &[Entry]) -> Layouts {
        let keep = |entry: &Entry| !entries.iter().any(|gone| gone.same(entry));
        let preload: Vec<(u32, String)> = self
            .preload
            .iter()
            .filter(|(_, data)| keep(&Entry::Preload(data.clone())))
            .cloned()
            .collect();
        let values: Vec<(List, String, String)> = self
            .values
            .iter()
            .filter(|(list, name, data)| keep(&Entry::Value(*list, name.clone(), data.clone())))
            .cloned()
            .collect();
        let keys = self
            .keys
            .iter()
            .copied()
            .filter(|list| {
                keep(&Entry::Key(*list))
                    || match list {
                        List::Preload => !preload.is_empty(),
                        _ => values.iter().any(|(other, _, _)| other == list),
                    }
            })
            .collect();
        Layouts {
            keys,
            preload,
            values,
        }
    }

    /// The writes that turn `self` into `target`, which `without` made from
    /// it. When values of `Preload` go, the rest are renumbered from 1 in
    /// their order, as Windows numbers them, so that it has no gaps.
    fn changes_to(&self, target: &Layouts) -> Vec<Change> {
        let mut changes = Vec::new();
        if target.preload.len() != self.preload.len() {
            for (index, (_, data)) in target.preload.iter().enumerate() {
                let number = index as u32 + 1;
                if !self.preload.iter().any(|(n, d)| *n == number && d == data) {
                    changes.push(Change::Set(number.to_string(), data.clone()));
                }
            }
            for (number, _) in &self.preload {
                if *number == 0 || *number as usize > target.preload.len() {
                    changes.push(Change::Remove(List::Preload, number.to_string()));
                }
            }
        }
        for value in &self.values {
            if !target.values.contains(value) {
                changes.push(Change::Remove(value.0, value.1.clone()));
            }
        }
        for list in &self.keys {
            if !target.keys.contains(list) {
                changes.push(Change::RemoveKey(*list));
            }
        }
        changes
    }

    fn write(&self, target: &Layouts) -> io::Result<()> {
        for change in self.changes_to(target) {
            match &change {
                Change::Set(name, data) => list_key(List::Preload)?.set_string(name, data)?,
                Change::Remove(list, name) => list_key(*list)?.remove_value(name)?,
                Change::RemoveKey(list) => {
                    let key = list_key(*list)?;
                    let empty = registry_snapshot::keys(&key)?.is_empty()
                        && registry_snapshot::values(&key)?.is_empty();
                    if !empty {
                        log::info!(
                            "Welcome screen: keeping {}, which is not empty",
                            list.path()
                        );
                        continue;
                    }
                    let (parent, name) = list.path().rsplit_once('\\').unwrap_or_default();
                    USERS
                        .options()
                        .read()
                        .write()
                        .open(format!(r".DEFAULT\{parent}"))?
                        .remove_tree(name)?;
                }
            }
            log::info!("Welcome screen: {change}");
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Change {
    /// `Preload` is the only list kbdi sets values of: it renumbers it.
    Set(String, String),
    Remove(List, String),
    RemoveKey(List),
}

impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Change::Set(name, data) => write!(f, r"set {}\{name}={data}", List::Preload.path()),
            Change::Remove(list, name) => write!(f, r"remove {}\{name}", list.path()),
            Change::RemoveKey(list) => write!(f, "remove {}", list.path()),
        }
    }
}

fn list_key(list: List) -> io::Result<windows_registry::Key> {
    Ok(USERS
        .options()
        .read()
        .write()
        .open(format!(r".DEFAULT\{}", list.path()))?)
}

/// Adds `additions` to `record` unless it holds them already.
fn merge(record: &mut Vec<Entry>, additions: impl IntoIterator<Item = Entry>) {
    for entry in additions {
        if !record.iter().any(|old| old.same(&entry)) {
            record.push(entry);
        }
    }
}

/// The keyboard that takes over an addition of `klid` of `lang_id`, the
/// dummy layout of that language: the first other KLID in `recorded`, the
/// KLIDs kbdi put on the welcome screen, that `layouts` still has under
/// `lang_id`. A keyboard of another language, as one added under its KLID's
/// LANGID by an older kbdi, takes over nothing, so that removing a keyboard
/// leaves `.DEFAULT` as it was before.
fn heir<'a>(
    layouts: &Layouts,
    klid: &str,
    recorded: &'a [String],
    lang_id: u16,
) -> Option<&'a str> {
    recorded.iter().map(String::as_str).find(|other| {
        !other.eq_ignore_ascii_case(klid) && layouts.lang_ids(other).contains(&lang_id)
    })
}

/// Splits `additions` into those `heir` takes over, by heir, and those to
/// remove. A key goes with the values of it that an heir takes, if any.
fn distribute<'a>(
    additions: Vec<Entry>,
    heir: impl Fn(u16) -> Option<&'a str>,
) -> (Vec<(&'a str, Vec<Entry>)>, Vec<Entry>) {
    let mut kept: Vec<(&str, Vec<Entry>)> = Vec::new();
    let mut gone = Vec::new();
    let (keys, values): (Vec<Entry>, Vec<Entry>) = additions
        .into_iter()
        .partition(|entry| matches!(entry, Entry::Key(_)));
    for entry in values {
        match entry.lang_id().and_then(&heir) {
            Some(heir) => match kept.iter_mut().find(|(name, _)| *name == heir) {
                Some((_, entries)) => entries.push(entry),
                None => kept.push((heir, vec![entry])),
            },
            None => gone.push(entry),
        }
    }
    for key in keys {
        match kept
            .iter_mut()
            .find(|(_, entries)| entries.iter().any(|entry| entry.list() == key.list()))
        {
            Some((_, entries)) => entries.push(key),
            None => gone.push(key),
        }
    }
    (kept, gone)
}

/// The welcome screen's input of the KLID under `lang_id`, `LLLL:KLID`.
fn input(lang_id: u16, klid: &str) -> io::Result<String> {
    let input = InputListItem::try_from(format!("{lang_id:04X}:{klid}").as_str())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid keyboard input ID"))?;
    Ok(format!("{}:{}", input.lcid(), input.kbid()))
}

/// Whether `lang_id` is transient: `LANG_NEUTRAL` with a sublanguage from
/// 8 up (`0x2000`, `0x2400`, …), assigned by each profile's language list.
fn is_transient(lang_id: u16) -> bool {
    lang_id >= 0x2000 && lang_id & 0x03FF == 0
}

/// `.DEFAULT`'s language list: each tag with the transient LANGID it
/// gives it, if any.
fn default_languages() -> io::Result<Vec<(String, Option<u16>)>> {
    let Ok(profile) = USERS.open(r".DEFAULT\Control Panel\International\User Profile") else {
        return Ok(Vec::new());
    };
    let mut languages = Vec::new();
    for tag in registry_snapshot::keys(&profile)? {
        let lang_id = profile
            .open(&tag)
            .and_then(|key| key.get_u32("TransientLangId"))
            .ok()
            .and_then(|value| u16::try_from(value).ok());
        languages.push((tag, lang_id));
    }
    Ok(languages)
}

/// What the welcome screen will call the layout of `tag` under `lang_id`,
/// by `.DEFAULT`'s language list, and whether that is a misnomer to warn
/// of. A LANGID that is not transient names its own language.
fn welcome_language(
    languages: &[(String, Option<u16>)],
    tag: &str,
    lang_id: u16,
) -> (bool, String) {
    if !is_transient(lang_id) {
        return (false, format!("{lang_id:04X} is not transient"));
    }
    let named = languages.iter().find(|(_, id)| *id == Some(lang_id));
    let own = languages
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(tag))
        .and_then(|(_, id)| *id);
    match (named, own) {
        (Some((name, _)), _) if name.eq_ignore_ascii_case(tag) => {
            (false, format!(".DEFAULT gives {lang_id:04X} to {name} too"))
        }
        (Some((name, _)), _) => (
            true,
            format!(
                ".DEFAULT gives {lang_id:04X} to {name}, so the welcome screen lists the layout under {name}"
            ),
        ),
        (None, Some(other)) => (
            true,
            format!(".DEFAULT gives {tag} {other:04X}, and {lang_id:04X} to no language"),
        ),
        (None, None) => (
            false,
            format!(".DEFAULT gives {lang_id:04X} to no language"),
        ),
    }
}

fn read_additions(klid: &str) -> io::Result<Option<Vec<Entry>>> {
    let Ok(key) = LOCAL_MACHINE.open(format!(r"{KEYBOARD_LAYOUTS}\{klid}")) else {
        return Ok(None);
    };
    let Ok(lines) = key.get_multi_string(ADDITIONS) else {
        return Ok(None);
    };
    let mut entries = Vec::new();
    for line in lines.iter().filter(|line| !line.is_empty()) {
        match Entry::parse(line) {
            Some(entry) => entries.push(entry),
            None => log::warn!("Ignoring {line:?} in {klid}'s {ADDITIONS:?}"),
        }
    }
    Ok(Some(entries))
}

fn write_additions(klid: &str, entries: &[Entry]) -> io::Result<()> {
    let lines: Vec<String> = entries.iter().map(ToString::to_string).collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    LOCAL_MACHINE
        .options()
        .read()
        .write()
        .open(format!(r"{KEYBOARD_LAYOUTS}\{klid}"))?
        .set_multi_string(ADDITIONS, &lines)?;
    Ok(())
}

/// The installed KLIDs whose layouts kbdi put on the welcome screen.
fn recorded_klids() -> io::Result<Vec<String>> {
    let layouts = LOCAL_MACHINE.open(KEYBOARD_LAYOUTS)?;
    let mut klids = Vec::new();
    for klid in registry_snapshot::keys(&layouts)? {
        if read_additions(&klid)?.is_some() {
            klids.push(klid);
        }
    }
    Ok(klids)
}

/// Adds the KLID's layout of `tag` to the welcome screen under `lang_id`,
/// the calling user's LANGID of the tag, and notes in its key what else
/// Windows added to `.DEFAULT`.
// [spec:kbdgen:req:tsf.register.welcome]
pub fn add(klid: &str, lang_id: u16, tag: &str) -> io::Result<()> {
    let layout = input(lang_id, klid)?;
    let (misnomer, language) = welcome_language(&default_languages()?, tag, lang_id);
    if misnomer {
        log::warn!("Welcome screen: adding {layout} for {tag}; {language}");
    } else {
        log::info!("Welcome screen: adding {layout} for {tag}; {language}");
    }
    let before = Layouts::read()?;
    let other = before.inputs(klid);
    if other.iter().any(|old| *old != layout) {
        log::warn!(
            "Welcome screen: {klid} is already there as {}",
            other.join(";")
        );
    }
    input::install(&layout, input::ILOT_DEFUSER4)?;
    let added = Layouts::read()?.added_since(&before);
    for entry in &added {
        log::info!("Welcome screen: Windows also added {entry}");
    }
    let mut additions = read_additions(klid)?.unwrap_or_default();
    merge(&mut additions, added);
    write_additions(klid, &additions)
}

/// Takes the KLID's layout off the welcome screen, under whichever LANGIDs
/// it is there, and with it the dummy layouts Windows added to `.DEFAULT`
/// doing so and, unless another keyboard kbdi put there under their LANGID
/// stays, when the layout was added.
// [spec:kbdgen:req:tsf.register.welcome]
pub fn remove(klid: &str) -> io::Result<()> {
    let additions = read_additions(klid)?;
    let mut layouts = Layouts::read()?;
    let mut remove = Vec::new();
    if layouts.mentions(klid) {
        let inputs = layouts.inputs(klid);
        if inputs.is_empty() {
            return Err(io::Error::other(format!(
                "the welcome screen names keyboard {klid} under no LANGID kbdi can read"
            )));
        }
        let inputs = inputs.join(";");
        log::info!("Welcome screen: removing {inputs}");
        input::install(&inputs, input::ILOT_DEFUSER4 | input::ILOT_UNINSTALL)?;
        let after = Layouts::read()?;
        if after.mentions(klid) {
            return Err(io::Error::other(format!(
                "Windows kept keyboard {klid} on the welcome screen"
            )));
        }
        remove = after.added_since(&layouts);
        layouts = after;
    }
    if let Some(additions) = additions.filter(|additions| !additions.is_empty()) {
        let recorded = recorded_klids()?;
        let (kept, gone) = distribute(additions, |lang_id| {
            heir(&layouts, klid, &recorded, lang_id)
        });
        for (heir, entries) in kept {
            log::info!(
                "Welcome screen: {heir} keeps what adding {klid} added of its language: {}",
                entries
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let mut record = read_additions(heir)?.unwrap_or_default();
            merge(&mut record, entries);
            write_additions(heir, &record)?;
        }
        merge(&mut remove, gone);
    }
    if remove.is_empty() {
        return Ok(());
    }
    layouts.write(&layouts.without(&remove))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layouts(preload: &[&str], substitutes: &[(&str, &str)], hidden: &[(&str, &str)]) -> Layouts {
        let values = |list, pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(move |(name, data)| (list, name.to_string(), data.to_string()))
                .collect::<Vec<_>>()
        };
        Layouts {
            keys: List::ALL.to_vec(),
            preload: preload
                .iter()
                .enumerate()
                .map(|(index, data)| (index as u32 + 1, data.to_string()))
                .collect(),
            values: [
                values(List::Substitutes, substitutes),
                values(List::HiddenDummyLayouts, hidden),
            ]
            .concat(),
        }
    }

    fn dummy(name: &str) -> Vec<Entry> {
        vec![
            Entry::Preload(name.into()),
            Entry::Value(List::Substitutes, name.into(), "00000409".into()),
            Entry::Value(List::HiddenDummyLayouts, name.into(), "00000409".into()),
        ]
    }

    /// `.DEFAULT` of Windows 10 LTSC before and after `2000:A0012000`: the
    /// layout under `00002000`, and the user's dummy layout of a profile-only
    /// language under `0x2400`.
    #[test]
    fn additions_are_what_windows_wrote_besides_the_layout() {
        let before = layouts(&["00000409", "00000809"], &[("00000809", "0000041d")], &[]);
        let after = layouts(
            &["00000409", "00000809", "00002000", "00002400"],
            &[
                ("00000809", "0000041d"),
                ("00002000", "a0012000"),
                ("00002400", "00000409"),
            ],
            &[("00002400", "00000409")],
        );
        assert_eq!(after.added_since(&before), dummy("00002400"));
        assert!(after.mentions("A0012000"));
        assert!(!before.mentions("a0012000"));
        // Removing the layout and its additions restores `.DEFAULT`.
        let removed = layouts(
            &["00000409", "00000809", "00002400"],
            &[("00000809", "0000041d"), ("00002400", "00000409")],
            &[("00002400", "00000409")],
        );
        assert_eq!(removed.added_since(&after), vec![]);
        assert_eq!(removed.without(&dummy("00002400")), before);
    }

    /// Windows 11: the user's dummy layout takes `00002000`, so the layout
    /// gets `d0012000`.
    #[test]
    fn additions_exclude_the_layout_under_any_substitute() {
        let before = layouts(&["00000409", "00000809"], &[("00000809", "0000041d")], &[]);
        let after = layouts(
            &["00000409", "00000809", "00002000", "d0012000"],
            &[
                ("00000809", "0000041d"),
                ("00002000", "00000409"),
                ("d0012000", "a0002000"),
            ],
            &[("00002000", "00000409")],
        );
        assert_eq!(after.added_since(&before), dummy("00002000"));
    }

    /// What the welcome screen had before, the same dummy layout, another
    /// keyboard of the same LANGID or en-US, is never an addition.
    #[test]
    fn existing_entries_are_not_additions() {
        let before = layouts(
            &["00000409", "00002000", "00002400"],
            &[("00002000", "a0012000"), ("00002400", "00000409")],
            &[("00002400", "00000409")],
        );
        let after = layouts(
            &["00000409", "00002000", "d0012000", "00002400"],
            &[
                ("00002000", "a0012000"),
                ("d0012000", "a0022000"),
                ("00002400", "00000409"),
            ],
            &[("00002400", "00000409")],
        );
        assert_eq!(after.added_since(&before), vec![]);
        // A value Windows changed is not an addition either.
        let changed = layouts(&["00000409"], &[("00002400", "00000407")], &[]);
        let before = layouts(&["00000409"], &[("00002400", "00000409")], &[]);
        assert_eq!(changed.added_since(&before), vec![]);
    }

    /// A key Windows created goes again once it is empty.
    #[test]
    fn created_keys_are_additions() {
        let mut before = layouts(&["00000409"], &[], &[]);
        before.keys.retain(|list| *list == List::Preload);
        let after = layouts(
            &["00000409", "00002000", "00002400"],
            &[("00002000", "a0012000"), ("00002400", "00000409")],
            &[("00002400", "00000409")],
        );
        let added = after.added_since(&before);
        assert_eq!(
            added,
            [
                vec![
                    Entry::Key(List::Substitutes),
                    Entry::Key(List::HiddenDummyLayouts)
                ],
                dummy("00002400")
            ]
            .concat()
        );
        // The substitute of a layout still there keeps its key.
        let target = after.without(&added);
        assert_eq!(target.keys, [List::Preload, List::Substitutes]);
        assert_eq!(
            after.changes_to(&target),
            vec![
                Change::Remove(List::Preload, "3".into()),
                Change::Remove(List::Substitutes, "00002400".into()),
                Change::Remove(List::HiddenDummyLayouts, "00002400".into()),
                Change::RemoveKey(List::HiddenDummyLayouts),
            ]
        );
    }

    #[test]
    fn removal_keeps_entries_whose_data_changed() {
        let state = layouts(
            &["00000409", "00002400"],
            &[("00002400", "a0002400")],
            &[("00002400", "00000409")],
        );
        let target = state.without(&dummy("00002400"));
        assert_eq!(
            target,
            layouts(&["00000409"], &[("00002400", "a0002400")], &[])
        );
    }

    #[test]
    fn preload_is_renumbered_in_order_without_gaps() {
        let state = layouts(
            &["00000409", "00002400", "00000407", "00000809"],
            &[("00002400", "00000409")],
            &[("00002400", "00000409")],
        );
        let target = state.without(&dummy("00002400"));
        assert_eq!(
            target
                .preload
                .iter()
                .map(|(_, data)| data.as_str())
                .collect::<Vec<_>>(),
            ["00000409", "00000407", "00000809"]
        );
        assert_eq!(
            state.changes_to(&target),
            vec![
                Change::Set("2".into(), "00000407".into()),
                Change::Set("3".into(), "00000809".into()),
                Change::Remove(List::Preload, "4".into()),
                Change::Remove(List::Substitutes, "00002400".into()),
                Change::Remove(List::HiddenDummyLayouts, "00002400".into()),
            ]
        );
        // Values before the removed one are left as they are.
        let last = layouts(&["00000409", "00000809", "00002400"], &[], &[]);
        assert_eq!(
            last.changes_to(&last.without(&dummy("00002400"))),
            vec![Change::Remove(List::Preload, "3".into())]
        );
        // Gaps kbdi did not make stay when it removes nothing from `Preload`.
        let mut gap = layouts(&["00000409", "00000809"], &[("00002400", "00000409")], &[]);
        gap.preload[1].0 = 3;
        assert_eq!(
            gap.changes_to(&gap.without(&dummy("00002400"))),
            vec![Change::Remove(List::Substitutes, "00002400".into())]
        );
        assert_eq!(state.changes_to(&state), vec![]);
    }

    #[test]
    fn additions_pass_to_another_keyboard_kbdi_put_on_the_welcome_screen() {
        let state = layouts(
            &["00000409", "d0022000", "00002000"],
            &[("d0022000", "a0022000"), ("00002000", "00000409")],
            &[("00002000", "00000409")],
        );
        let recorded = [
            "a0012000".to_owned(),
            "a0032000".to_owned(),
            "a0022000".to_owned(),
        ];
        assert_eq!(
            heir(&state, "a0012000", &recorded, 0x2000),
            Some("a0022000")
        );
        // Not to itself, nor to one no longer there.
        let state = layouts(&["00000409", "00002000"], &[("00002000", "00000409")], &[]);
        assert_eq!(heir(&state, "a0012000", &recorded, 0x2000), None);
        let mut record = dummy("00002000");
        merge(
            &mut record,
            dummy("00002000").into_iter().chain(dummy("00002400")),
        );
        assert_eq!(record, [dummy("00002000"), dummy("00002400")].concat());
    }

    #[test]
    fn additions_round_trip_through_text() {
        let entries = [
            vec![Entry::Key(List::HiddenDummyLayouts)],
            dummy("00002400"),
        ]
        .concat();
        for entry in entries {
            assert_eq!(Entry::parse(&entry.to_string()), Some(entry));
        }
        assert_eq!(
            dummy("00002400")[1].to_string(),
            r"Keyboard Layout\Substitutes\00002400=00000409"
        );
        for bad in [
            "",
            r"Keyboard Layout\Preload=",
            r"Keyboard Layout\Preload\3=00002400",
            r"Keyboard Layout\Substitutes=00000409",
            r"Keyboard Layout\Substitutes\=00000409",
            r"Keyboard Layout\Toggle\Hotkey=1",
            r"Keyboard Layout\Toggle",
            "00002400",
        ] {
            assert_eq!(Entry::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn welcome_screen_inputs_take_the_given_lang_id() {
        assert_eq!(input(0x2400, "a0002000").unwrap(), "2400:A0002000");
        assert_eq!(input(0x043B, "A001043b").unwrap(), "043B:A001043B");
        assert!(input(0x2400, "a00").is_err());
    }

    /// A layout added under the KLID's LANGID, as kbdi did before, under
    /// the user's LANGID beside its dummy layout, as on Windows 11, and
    /// both: removal finds each LANGID.
    #[test]
    fn inputs_are_found_under_the_lang_ids_they_were_added_under() {
        let old = layouts(
            &["00000409", "00002000", "00002400"],
            &[("00002000", "a0002000"), ("00002400", "00000409")],
            &[("00002400", "00000409")],
        );
        assert_eq!(old.inputs("A0002000"), ["2000:A0002000"]);
        let new = layouts(
            &["00000409", "00002000", "00002400", "d0012400"],
            &[
                ("00002000", "a0012000"),
                ("00002400", "00000409"),
                ("d0012400", "a0002000"),
            ],
            &[("00002400", "00000409")],
        );
        assert_eq!(new.inputs("a0002000"), ["2400:A0002000"]);
        assert_eq!(new.inputs("a0012000"), ["2000:A0012000"]);
        let both = layouts(
            &["00000409", "00002000", "d0012400"],
            &[("00002000", "a0002000"), ("d0012400", "a0002000")],
            &[],
        );
        assert_eq!(both.inputs("a0002000"), ["2000:A0002000", "2400:A0002000"]);
        // A layout Windows lists by its own KLID, and none at all.
        let direct = layouts(&["00000409", "0000043b"], &[], &[]);
        assert_eq!(direct.inputs("0000043B"), ["043B:0000043B"]);
        assert!(direct.inputs("a0002000").is_empty());
        assert!(!direct.mentions("a0002000"));
        // A substitute name kbdi cannot read is no input.
        let odd = layouts(&["00000409"], &[("2400", "a0002000")], &[]);
        assert!(odd.mentions("a0002000") && odd.inputs("a0002000").is_empty());
    }

    /// Windows 10: the user's dummy layout of the profile-only language,
    /// `00002400`, comes with the layout under the same LANGID, which then
    /// takes a `d0` name. Neither the dummy layout the welcome screen had
    /// for another keyboard nor the layout is an addition.
    #[test]
    fn layout_under_the_user_lang_id_beside_its_dummy_layout() {
        let before = layouts(
            &["00000409", "00000809", "00002000", "00002400"],
            &[
                ("00000809", "0000041d"),
                ("00002000", "a0002000"),
                ("00002400", "00000409"),
            ],
            &[("00002400", "00000409")],
        );
        let after = layouts(
            &[
                "00000409", "00000809", "00002000", "00002400", "00002800", "d0012800",
            ],
            &[
                ("00000809", "0000041d"),
                ("00002000", "a0002000"),
                ("00002400", "00000409"),
                ("00002800", "00000409"),
                ("d0012800", "a0012000"),
            ],
            &[("00002400", "00000409"), ("00002800", "00000409")],
        );
        assert_eq!(after.inputs("a0012000"), ["2800:A0012000"]);
        assert_eq!(after.added_since(&before), dummy("00002800"));
        let removed = layouts(
            &["00000409", "00000809", "00002000", "00002400", "00002800"],
            &[
                ("00000809", "0000041d"),
                ("00002000", "a0002000"),
                ("00002400", "00000409"),
                ("00002800", "00000409"),
            ],
            &[("00002400", "00000409"), ("00002800", "00000409")],
        );
        assert_eq!(removed.added_since(&after), vec![]);
        assert_eq!(removed.without(&dummy("00002800")), before);
    }

    /// Windows 10: a second keyboard under `0x2800`, of a lower KLID,
    /// takes `d0012800` and the first moves to `d0022800`. That is no
    /// addition, and neither is its removal, which moves it back.
    #[test]
    fn layouts_windows_names_anew_are_not_additions() {
        let preload = ["00000409", "00002000", "00002400", "00002800", "d0012800"];
        let hidden = [("00002400", "00000409"), ("00002800", "00000409")];
        let one = layouts(
            &preload,
            &[
                ("00002000", "a0002000"),
                ("00002400", "00000409"),
                ("00002800", "00000409"),
                ("d0012800", "a0012000"),
            ],
            &hidden,
        );
        let two = layouts(
            &[&preload[..], &["d0022800"]].concat(),
            &[
                ("00002000", "a0002000"),
                ("00002400", "00000409"),
                ("00002800", "00000409"),
                ("d0012800", "a0003000"),
                ("d0022800", "a0012000"),
            ],
            &hidden,
        );
        assert_eq!(two.added_since(&one), vec![]);
        assert_eq!(one.added_since(&two), vec![]);
        assert_eq!(two.inputs("a0012000"), ["2800:A0012000"]);
        assert_eq!(two.inputs("a0003000"), ["2800:A0003000"]);
    }

    /// The Võro keyboard of an older kbdi is under `0x2000`, so it takes
    /// over no dummy layout of `0x2800`; a keyboard under `0x2800` does.
    #[test]
    fn additions_pass_only_to_a_keyboard_of_their_language() {
        let state = layouts(
            &["00000409", "00002000", "00002400", "00002800", "d0012800"],
            &[
                ("00002000", "a0002000"),
                ("00002400", "00000409"),
                ("00002800", "00000409"),
                ("d0012800", "a0003000"),
            ],
            &[("00002400", "00000409"), ("00002800", "00000409")],
        );
        let recorded = [
            "a0002000".to_owned(),
            "a0003000".to_owned(),
            "a0012000".to_owned(),
        ];
        assert_eq!(
            heir(&state, "a0012000", &recorded, 0x2800),
            Some("a0003000")
        );
        assert_eq!(heir(&state, "a0012000", &recorded, 0x2400), None);
        assert_eq!(heir(&state, "a0003000", &recorded, 0x2800), None);
        let additions = [
            vec![Entry::Key(List::HiddenDummyLayouts)],
            dummy("00002800"),
            dummy("00002400"),
        ]
        .concat();
        let (kept, gone) = distribute(additions.clone(), |lang_id| {
            heir(&state, "a0012000", &recorded, lang_id)
        });
        assert_eq!(
            kept,
            vec![(
                "a0003000",
                [
                    dummy("00002800"),
                    vec![Entry::Key(List::HiddenDummyLayouts)]
                ]
                .concat()
            )]
        );
        assert_eq!(gone, dummy("00002400"));
        // Without an heir, all go, keys too.
        let (kept, gone) = distribute(additions.clone(), |_| None);
        assert!(kept.is_empty());
        assert_eq!(gone.len(), additions.len());
        assert_eq!(gone.last(), Some(&Entry::Key(List::HiddenDummyLayouts)));
    }

    #[test]
    fn transient_lang_ids_are_neutral_with_a_high_sublanguage() {
        for lang_id in [0x2000, 0x2400, 0x2800, 0x5000] {
            assert!(is_transient(lang_id), "{lang_id:04X}");
        }
        for lang_id in [0x0409, 0x0C3B, 0x1000, 0x0C00, 0x1400, 0x2001, 0x203B] {
            assert!(!is_transient(lang_id), "{lang_id:04X}");
        }
    }

    /// `.DEFAULT` of Windows 10 LTSC, copied from a user with en-SE first.
    #[test]
    fn welcome_language_is_named_by_the_default_language_list() {
        let languages = vec![
            ("en-SE".to_owned(), Some(0x2000)),
            ("en-US".to_owned(), None),
            ("en-GB".to_owned(), None),
        ];
        let (misnomer, text) = welcome_language(&languages, "vro-Latn", 0x2000);
        assert!(misnomer);
        assert!(text.ends_with("under en-SE"), "{text}");
        assert_eq!(
            welcome_language(&languages, "vro-Latn", 0x2400),
            (false, ".DEFAULT gives 2400 to no language".into())
        );
        assert!(!welcome_language(&languages, "se-FI", 0x0C3B).0);
        let copied = [languages, vec![("vro-Latn".to_owned(), Some(0x2400))]].concat();
        assert!(!welcome_language(&copied, "VRO-latn", 0x2400).0);
        let (misnomer, text) = welcome_language(&copied, "vro-Latn", 0x2800);
        assert!(misnomer);
        assert!(text.contains("vro-Latn 2400"), "{text}");
    }
}
