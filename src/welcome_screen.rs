//! The welcome screen's keyboard layouts, in `HKEY_USERS\.DEFAULT`
//! (kbdgen's `tsf.register.welcome`).
//!
//! `InstallLayoutOrTip` with `ILOT_DEFUSER4`, whether it adds or removes a
//! layout, also writes the calling user's hidden dummy layouts into
//! `.DEFAULT`. Windows gives each of a user's languages whose inputs are all
//! text service profiles such a layout: `0000LLLL` in `Preload`, substituted
//! by a real layout and listed under `CTF\HiddenDummyLayouts`. Removing the
//! keyboard's layout leaves them behind. kbdi therefore compares `.DEFAULT`
//! before and after each call. It keeps what enabling added besides the
//! layout in the KLID's key, and removes it, together with anything removing
//! the layout added, when the keyboard leaves the welcome screen.
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

impl Entry {
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

    /// What `self` has that `before` lacks, except the KLID's own entries:
    /// its substitute and the `Preload` value naming it or that substitute.
    fn added_since(&self, before: &Layouts, klid: &str) -> Vec<Entry> {
        let substitutes: Vec<&str> = self
            .values
            .iter()
            .filter(|(list, _, data)| *list == List::Substitutes && data.eq_ignore_ascii_case(klid))
            .map(|(_, name, _)| name.as_str())
            .collect();
        let own = |entry: &Entry| match entry {
            Entry::Key(_) => false,
            Entry::Preload(data) => {
                data.eq_ignore_ascii_case(klid)
                    || substitutes
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(data))
            }
            Entry::Value(_, _, data) => data.eq_ignore_ascii_case(klid),
        };
        self.entries()
            .filter(|entry| !own(entry) && !entry.existed_in(before))
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

/// The keyboard that takes over `klid`'s additions while it is still on the
/// welcome screen: the first other KLID in `recorded`, the KLIDs kbdi put on
/// the welcome screen, that `layouts` still names.
fn heir<'a>(layouts: &Layouts, klid: &str, recorded: &'a [String]) -> Option<&'a str> {
    recorded
        .iter()
        .map(String::as_str)
        .find(|other| !other.eq_ignore_ascii_case(klid) && layouts.mentions(other))
}

/// The welcome screen's input for a KLID: under the LANGID of the KLID's
/// low word, which is the language `install` created it for.
fn layout(klid: &str) -> io::Result<String> {
    let input = InputListItem::try_from(format!("{}:{klid}", klid.get(4..).unwrap_or("")).as_str())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid keyboard input ID"))?;
    Ok(format!("{}:{}", input.lcid(), input.kbid()))
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

/// Adds the KLID's layout to the welcome screen and notes in its key what
/// else Windows added to `.DEFAULT`.
// [spec:kbdgen:req:tsf.register.welcome]
pub fn add(klid: &str) -> io::Result<()> {
    let before = Layouts::read()?;
    input::install(&layout(klid)?, input::ILOT_DEFUSER4)?;
    let added = Layouts::read()?.added_since(&before, klid);
    for entry in &added {
        log::info!("Welcome screen: Windows also added {entry}");
    }
    let mut additions = read_additions(klid)?.unwrap_or_default();
    merge(&mut additions, added);
    write_additions(klid, &additions)
}

/// Takes the KLID's layout off the welcome screen, and with it what Windows
/// added to `.DEFAULT` doing so and, unless another keyboard kbdi put there
/// stays, when the layout was added.
// [spec:kbdgen:req:tsf.register.welcome]
pub fn remove(klid: &str) -> io::Result<()> {
    let additions = read_additions(klid)?;
    let mut layouts = Layouts::read()?;
    let mut remove = Vec::new();
    if layouts.mentions(klid) {
        input::install(&layout(klid)?, input::ILOT_DEFUSER4 | input::ILOT_UNINSTALL)?;
        let after = Layouts::read()?;
        if after.mentions(klid) {
            return Err(io::Error::other(format!(
                "Windows kept keyboard {klid} on the welcome screen"
            )));
        }
        remove = after.added_since(&layouts, klid);
        layouts = after;
    }
    if let Some(additions) = additions.filter(|additions| !additions.is_empty()) {
        let recorded = recorded_klids()?;
        match heir(&layouts, klid, &recorded) {
            Some(heir) => {
                log::info!("Welcome screen: {heir} keeps what adding {klid} added");
                let mut kept = read_additions(heir)?.unwrap_or_default();
                merge(&mut kept, additions);
                write_additions(heir, &kept)?;
            }
            None => merge(&mut remove, additions),
        }
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
        assert_eq!(after.added_since(&before, "a0012000"), dummy("00002400"));
        assert!(after.mentions("A0012000"));
        assert!(!before.mentions("a0012000"));
        // Removing the layout and its additions restores `.DEFAULT`.
        let removed = layouts(
            &["00000409", "00000809", "00002400"],
            &[("00000809", "0000041d"), ("00002400", "00000409")],
            &[("00002400", "00000409")],
        );
        assert_eq!(removed.added_since(&after, "a0012000"), vec![]);
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
        assert_eq!(after.added_since(&before, "a0002000"), dummy("00002000"));
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
        assert_eq!(after.added_since(&before, "a0022000"), vec![]);
        // A value Windows changed is not an addition either.
        let changed = layouts(&["00000409"], &[("00002400", "00000407")], &[]);
        let before = layouts(&["00000409"], &[("00002400", "00000409")], &[]);
        assert_eq!(changed.added_since(&before, "a0022000"), vec![]);
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
        let added = after.added_since(&before, "a0012000");
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
        assert_eq!(heir(&state, "a0012000", &recorded), Some("a0022000"));
        // Not to itself, nor to one no longer there.
        let state = layouts(&["00000409", "00002000"], &[("00002000", "00000409")], &[]);
        assert_eq!(heir(&state, "a0012000", &recorded), None);
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
    fn welcome_screen_layout_uses_the_klid_language() {
        assert_eq!(layout("a0002000").unwrap(), "2000:A0002000");
        assert_eq!(layout("A001043b").unwrap(), "043B:A001043B");
        assert!(layout("a00").is_err());
    }
}
