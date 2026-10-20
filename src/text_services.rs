//! CLI session verification and recovery after enabling custom keyboards.
use crate::{
    keyboard::KeyboardRegKey,
    platform::{core_profiles, text_session},
    tsf::{self, TipInput},
    types::InputListItem,
};
use std::{
    io::{self, Read},
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// The first line of the probe's output: the live profiles follow, one per
/// line.
const PROFILES: &str = "KBDI-PROFILES-1";
/// The first line of the probe's output when this Windows lacks the live
/// contract: why follows.
const UNSUPPORTED: &str = "KBDI-PROFILES-UNSUPPORTED";

/// Internal CLI child entry point. Does not initialize the logger or change
/// keyboard state. The parent bounds its lifetime and validates its protocol.
pub fn probe() -> io::Result<()> {
    match core_profiles::enabled() {
        Ok(profiles) => {
            println!("{PROFILES}");
            for profile in profiles {
                println!("{profile}");
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::Unsupported => {
            println!("{UNSUPPORTED}");
            println!("{error}");
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// What the probe saw: the live profiles, or why this Windows has none to
/// show.
#[derive(Debug, PartialEq)]
enum Snapshot {
    Live(Vec<String>),
    Unsupported(String),
}

fn parse_snapshot(text: &str) -> io::Result<Snapshot> {
    let mut lines = text.lines();
    match lines.next() {
        Some(PROFILES) => {
            let profiles: Vec<_> = lines.map(str::to_owned).collect();
            if profiles.is_empty()
                || profiles.len() > 256
                || profiles.iter().any(|s| s.len() > 256 || !s.contains(':'))
            {
                return Err(io::Error::other("invalid live-profile snapshot"));
            }
            Ok(Snapshot::Live(profiles))
        }
        Some(UNSUPPORTED) => {
            let reason = lines.collect::<Vec<_>>().join(" ");
            if reason.trim().is_empty() || reason.len() > 1024 {
                return Err(io::Error::other("invalid live-profile probe response"));
            }
            Ok(Snapshot::Unsupported(reason))
        }
        _ => Err(io::Error::other("invalid live-profile probe response")),
    }
}

fn read_live_profiles() -> io::Result<Snapshot> {
    let mut child = Command::new(std::env::current_exe()?)
        .arg("__keyboard_profiles")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
        .spawn()?;
    // Drain both pipes while waiting: a large profile list must not block the
    // child on a full stdout pipe and be mistaken for a Windows API timeout.
    fn drain(stream: impl Read + Send + 'static) -> std::thread::JoinHandle<io::Result<Vec<u8>>> {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stream.take(128 * 1024).read_to_end(&mut bytes)?;
            Ok(bytes)
        })
    }
    let stdout = drain(child.stdout.take().expect("piped stdout"));
    let stderr = drain(child.stderr.take().expect("piped stderr"));
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            let _ = stdout.join();
            let _ = stderr.join();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "live input-profile probe timed out; no verified recovery",
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let status = child.wait()?;
    let stdout = stdout
        .join()
        .map_err(|_| io::Error::other("profile stdout reader failed"))??;
    let stderr = stderr
        .join()
        .map_err(|_| io::Error::other("profile stderr reader failed"))??;
    if !status.success() {
        return Err(io::Error::other(format!(
            "live input-profile verification unavailable ({}): {}",
            status,
            String::from_utf8_lossy(&stderr).trim()
        )));
    }
    parse_snapshot(&String::from_utf8(stdout).map_err(io::Error::other)?)
}

fn missing(expected: &[String], actual: &[String]) -> Vec<String> {
    expected
        .iter()
        .filter(|id| !actual.iter().any(|live| tsf::same_input(id, live)))
        .cloned()
        .collect()
}

/// How `ensure_profiles` left the live profiles.
#[derive(Debug, PartialEq)]
enum Outcome {
    /// They were current.
    Current,
    /// They were current after one restart of ctfmon.
    Refreshed,
    /// This Windows cannot show them, for the reason given.
    Unverified(String),
}

/// The profiles of a snapshot taken after the first, which must be live: a
/// Windows does not lose the contract between two probes.
fn live(snapshot: Snapshot) -> io::Result<Vec<String>> {
    match snapshot {
        Snapshot::Live(profiles) => Ok(profiles),
        Snapshot::Unsupported(reason) => Err(io::Error::other(format!(
            "live input-profile verification became unavailable: {reason}"
        ))),
    }
}

// A failed probe is not evidence that restarting is appropriate, nor is a
// Windows without the live contract. A completed snapshot with
// missing/mismatched IDs is; retry delivery before one restart.
fn ensure_profiles(
    expected: &[String],
    check_only: bool,
    mut probe: impl FnMut() -> io::Result<Snapshot>,
    mut restart: impl FnMut() -> io::Result<()>,
    mut pause: impl FnMut(),
) -> io::Result<Outcome> {
    if expected.is_empty() {
        return Ok(Outcome::Current);
    }
    let mut absent = Vec::new();
    for attempt in 0..3 {
        let actual = match probe()? {
            Snapshot::Unsupported(reason) if attempt == 0 => {
                return Ok(Outcome::Unverified(reason));
            }
            snapshot => live(snapshot)?,
        };
        absent = missing(expected, &actual);
        if absent.is_empty() {
            return Ok(Outcome::Current);
        }
        log::debug!("Live input profiles: {actual:?}; missing: {absent:?}");
        if attempt < 2 {
            pause();
        }
    }
    if check_only {
        return Err(io::Error::other(format!(
            "stale live keyboard profiles: {}",
            absent.join(", ")
        )));
    }
    restart()?;
    for attempt in 0..3 {
        let actual = live(probe()?)?;
        absent = missing(expected, &actual);
        if absent.is_empty() {
            return Ok(Outcome::Refreshed);
        }
        if attempt < 2 {
            pause();
        }
    }
    Err(io::Error::other(format!(
        "text services restarted but live profiles remain stale: {}; run keyboard_refresh in your desktop session or sign out and back in",
        absent.join(", ")
    )))
}

/// What `refresh` logs for `outcome` of `count` inputs. `--check` promises
/// verification, so it fails when this Windows cannot verify. Otherwise an
/// unverified outcome succeeds without a restart: `InstallLayoutOrTip`
/// applies an input to the running session, and on Windows 10 19044, which
/// lacks the contract, processes that ran before enabling list and activate
/// new profiles and layouts while ctfmon keeps running.
fn report(outcome: &Outcome, check_only: bool, count: usize) -> io::Result<String> {
    Ok(match outcome {
        Outcome::Current => {
            format!(
                "Text services already current; verified {count} enabled custom keyboard profile(s)"
            )
        }
        Outcome::Refreshed => {
            format!("Text services refreshed; verified {count} enabled custom keyboard profile(s)")
        }
        Outcome::Unverified(reason) if check_only => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("live keyboard verification is unavailable on this Windows: {reason}"),
            ));
        }
        Outcome::Unverified(reason) => format!(
            "Live keyboard verification skipped: this Windows does not offer it ({reason}); \
             {count} enabled custom keyboard input(s) are saved, and text services were not restarted"
        ),
    })
}

/// Verify every enabled custom layout owned by kbdi, optionally refreshing
/// ctfmon once. Must be called by kbdi itself (the isolated probe is a CLI mode).
pub fn refresh(check_only: bool) -> io::Result<()> {
    let session = text_session::Session::current()?;
    let before = crate::enabled_keyboards()?;
    let installed = KeyboardRegKey::installed();
    // A profile is live only while its text service is registered.
    let profiles: Vec<u128> = if tsf::available() {
        installed
            .iter()
            .filter_map(|layout| {
                layout
                    .product_code()
                    .and_then(|code| tsf::parse_guid(&code))
            })
            .collect()
    } else {
        Vec::new()
    };
    // [spec:kbdgen:req:tsf.register.enable]
    let expected: Vec<String> = before
        .iter()
        .flat_map(|(_, ids)| ids)
        .filter_map(|id| {
            if let Some(tip) = TipInput::parse(id) {
                return (tip.clsid == tsf::CLSID && profiles.contains(&tip.profile))
                    .then(|| id.clone());
            }
            let input = InputListItem::try_from(id.as_str()).ok()?;
            installed
                .iter()
                .any(|layout| layout.regkey_id().eq_ignore_ascii_case(&input.kbid()))
                .then(|| id.clone())
        })
        .collect();
    let mut saved_selection = None;
    let result = ensure_profiles(
        &expected,
        check_only,
        read_live_profiles,
        || {
            saved_selection = Some(text_session::ActiveKeyboard::capture());
            session.restart()
        },
        || std::thread::sleep(Duration::from_millis(500)),
    );
    drop(saved_selection);
    let outcome = result?;
    if crate::enabled_keyboards()? != before {
        return Err(io::Error::other(
            "input preferences changed during text-services recovery; verify the user's keyboard list",
        ));
    }
    log::info!("{}", report(&outcome, check_only, expected.len())?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn expected() -> Vec<String> {
        vec!["2400:A0003800".into(), "2400:A0013800".into()]
    }
    fn profiles(profiles: &[&str]) -> io::Result<Snapshot> {
        Ok(Snapshot::Live(
            profiles.iter().map(|s| s.to_string()).collect(),
        ))
    }
    fn stale() -> io::Result<Snapshot> {
        profiles(&["2400:00000000"])
    }
    fn unsupported() -> io::Result<Snapshot> {
        Ok(Snapshot::Unsupported(
            "ICoreKeyboardInputProfile unavailable".into(),
        ))
    }
    #[test]
    fn healthy_batch_does_not_restart_and_matching_is_case_insensitive() {
        assert_eq!(
            ensure_profiles(
                &expected(),
                false,
                || profiles(&["2400:a0003800", "2400:a0013800", "0409:00000409"]),
                || panic!("healthy session must not restart"),
                || {}
            )
            .unwrap(),
            Outcome::Current
        );
    }
    #[test]
    fn zero_layout_id_recovers_entire_batch_with_one_restart() {
        let mut probes = 0;
        let mut restarts = 0;
        assert_eq!(
            ensure_profiles(
                &expected(),
                false,
                || {
                    probes += 1;
                    if probes <= 3 {
                        stale()
                    } else {
                        Ok(Snapshot::Live(expected()))
                    }
                },
                || {
                    restarts += 1;
                    Ok(())
                },
                || {}
            )
            .unwrap(),
            Outcome::Refreshed
        );
        assert_eq!(restarts, 1);
        assert_eq!(probes, 4);
    }
    #[test]
    fn delayed_delivery_and_check_only_never_restart() {
        let mut probes = 0;
        assert_eq!(
            ensure_profiles(
                &expected(),
                false,
                || {
                    probes += 1;
                    if probes == 1 {
                        stale()
                    } else {
                        Ok(Snapshot::Live(expected()))
                    }
                },
                || panic!(),
                || {}
            )
            .unwrap(),
            Outcome::Current
        );
        assert!(ensure_profiles(&expected(), true, stale, || panic!(), || {}).is_err());
    }
    #[test]
    fn text_service_profiles_match_in_either_form() {
        let tip = TipInput::new(0x2000, 0x94C7_1262_EE9D_489B_926C_1593_8155_8D90).to_string();
        let live = format!("0x{}", tip.to_ascii_lowercase());
        assert!(missing(std::slice::from_ref(&tip), &[live]).is_empty());
        assert_eq!(missing(std::slice::from_ref(&tip), &expected()), vec![tip]);
    }
    #[test]
    fn probe_failure_is_not_a_reason_to_restart() {
        assert!(
            ensure_profiles(
                &expected(),
                false,
                || Err(io::ErrorKind::Unsupported.into()),
                || panic!(),
                || {}
            )
            .is_err()
        );
        assert_eq!(
            ensure_profiles(&[], false, || panic!(), || panic!(), || {}).unwrap(),
            Outcome::Current
        );
    }
    /// Windows 10 19044: the class exists, its profiles lack the interface.
    #[test]
    fn windows_without_the_contract_is_unverified_without_restart_or_retry() {
        for check_only in [false, true] {
            let mut probes = 0;
            assert_eq!(
                ensure_profiles(
                    &expected(),
                    check_only,
                    || {
                        probes += 1;
                        unsupported()
                    },
                    || panic!("no evidence for a restart"),
                    || panic!("nothing to wait for")
                )
                .unwrap(),
                Outcome::Unverified("ICoreKeyboardInputProfile unavailable".into())
            );
            assert_eq!(probes, 1);
        }
    }
    #[test]
    fn losing_the_contract_between_probes_fails() {
        let mut probes = 0;
        assert!(
            ensure_profiles(
                &expected(),
                false,
                || {
                    probes += 1;
                    if probes == 1 { stale() } else { unsupported() }
                },
                || panic!(),
                || {}
            )
            .is_err()
        );
        let mut probes = 0;
        assert!(
            ensure_profiles(
                &expected(),
                false,
                || {
                    probes += 1;
                    if probes <= 3 { stale() } else { unsupported() }
                },
                || Ok(()),
                || {}
            )
            .is_err()
        );
    }
    #[test]
    fn recovery_failure_and_persistently_stale_profiles_fail() {
        assert!(
            ensure_profiles(
                &expected(),
                false,
                stale,
                || Err(io::ErrorKind::PermissionDenied.into()),
                || {}
            )
            .is_err()
        );
        let mut restarts = 0;
        assert!(
            ensure_profiles(
                &expected(),
                false,
                stale,
                || {
                    restarts += 1;
                    Ok(())
                },
                || {}
            )
            .is_err()
        );
        assert_eq!(restarts, 1);
    }
    #[test]
    fn probe_output_parses_strictly() {
        assert_eq!(
            parse_snapshot("KBDI-PROFILES-1\r\n2400:A0003800\r\n0409:00000409\r\n").unwrap(),
            Snapshot::Live(vec!["2400:A0003800".into(), "0409:00000409".into()])
        );
        assert_eq!(
            parse_snapshot("KBDI-PROFILES-UNSUPPORTED\nICoreKeyboardInputProfile {29FC1D95-E12C-43D9-98A5-DE79A87EA36B} unavailable: No such interface supported (0x80004002)\n").unwrap(),
            Snapshot::Unsupported("ICoreKeyboardInputProfile {29FC1D95-E12C-43D9-98A5-DE79A87EA36B} unavailable: No such interface supported (0x80004002)".into())
        );
        let long = format!("KBDI-PROFILES-1\n{}:x", "0".repeat(256));
        for bad in [
            "",
            "KBDI-PROFILES-1",
            "KBDI-PROFILES-1\nnot a profile",
            long.as_str(),
            "KBDI-PROFILES-UNSUPPORTED",
            "KBDI-PROFILES-UNSUPPORTED\n \n",
            "KBDI-PROFILES-2\n2400:A0003800",
            "2400:A0003800",
        ] {
            assert!(parse_snapshot(bad).is_err(), "{bad:?}");
        }
    }
    #[test]
    fn unverified_succeeds_except_when_only_checking() {
        let unverified = Outcome::Unverified("ICoreKeyboardInputProfile unavailable".into());
        let message = report(&unverified, false, 2).unwrap();
        assert!(
            message.starts_with("Live keyboard verification skipped"),
            "{message}"
        );
        assert!(message.contains("ICoreKeyboardInputProfile unavailable"));
        assert_eq!(
            report(&unverified, true, 2).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        for outcome in [Outcome::Current, Outcome::Refreshed] {
            assert!(report(&outcome, true, 2).unwrap().contains("verified 2"));
        }
    }
}
