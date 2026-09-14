//! CLI session verification and recovery after enabling custom keyboards.
use crate::{
    keyboard::KeyboardRegKey,
    platform::{core_profiles, text_session},
    types::InputListItem,
};
use std::{
    io::{self, Read},
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Internal CLI child entry point. Does not initialize the logger or change
/// keyboard state. The parent bounds its lifetime and validates its protocol.
pub fn probe() -> io::Result<()> {
    let profiles = core_profiles::enabled()?;
    println!("KBDI-PROFILES-1");
    for profile in profiles {
        println!("{profile}");
    }
    Ok(())
}

fn read_live_profiles() -> io::Result<Vec<String>> {
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
    let text = String::from_utf8(stdout).map_err(io::Error::other)?;
    let mut lines = text.lines();
    if lines.next() != Some("KBDI-PROFILES-1") {
        return Err(io::Error::other("invalid live-profile probe response"));
    }
    let profiles: Vec<_> = lines.map(str::to_owned).collect();
    if profiles.is_empty()
        || profiles.len() > 256
        || profiles.iter().any(|s| s.len() > 256 || !s.contains(':'))
    {
        return Err(io::Error::other("invalid live-profile snapshot"));
    }
    Ok(profiles)
}

fn missing(expected: &[String], actual: &[String]) -> Vec<String> {
    expected
        .iter()
        .filter(|id| !actual.iter().any(|live| id.eq_ignore_ascii_case(live)))
        .cloned()
        .collect()
}

// A failed probe is not evidence that restarting is appropriate. A completed
// snapshot with missing/mismatched IDs is; retry delivery before one restart.
fn ensure_profiles(
    expected: &[String],
    check_only: bool,
    mut probe: impl FnMut() -> io::Result<Vec<String>>,
    mut restart: impl FnMut() -> io::Result<()>,
    mut pause: impl FnMut(),
) -> io::Result<bool> {
    if expected.is_empty() {
        return Ok(false);
    }
    let mut absent = Vec::new();
    for attempt in 0..3 {
        let actual = probe()?;
        absent = missing(expected, &actual);
        if absent.is_empty() {
            return Ok(false);
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
        let actual = probe()?;
        absent = missing(expected, &actual);
        if absent.is_empty() {
            return Ok(true);
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

/// Verify every enabled custom layout owned by kbdi, optionally refreshing
/// ctfmon once. Must be called by kbdi itself (the isolated probe is a CLI mode).
pub fn refresh(check_only: bool) -> io::Result<()> {
    let session = text_session::Session::current()?;
    let before = crate::enabled_keyboards()?;
    let installed = KeyboardRegKey::installed();
    let expected: Vec<String> = before
        .iter()
        .flat_map(|(_, ids)| ids)
        .filter_map(|id| {
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
    let restarted = result?;
    if crate::enabled_keyboards()? != before {
        return Err(io::Error::other(
            "input preferences changed during text-services recovery; verify the user's keyboard list",
        ));
    }
    log::info!(
        "{}; verified {} enabled custom keyboard profile(s)",
        if restarted {
            "Text services refreshed"
        } else {
            "Text services already current"
        },
        expected.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn expected() -> Vec<String> {
        vec!["2400:A0003800".into(), "2400:A0013800".into()]
    }
    #[test]
    fn healthy_batch_does_not_restart_and_matching_is_case_insensitive() {
        assert!(
            !ensure_profiles(
                &expected(),
                false,
                || Ok(vec![
                    "2400:a0003800".into(),
                    "2400:a0013800".into(),
                    "0409:00000409".into()
                ]),
                || panic!("healthy session must not restart"),
                || {}
            )
            .unwrap()
        );
    }
    #[test]
    fn zero_layout_id_recovers_entire_batch_with_one_restart() {
        let mut probes = 0;
        let mut restarts = 0;
        assert!(
            ensure_profiles(
                &expected(),
                false,
                || {
                    probes += 1;
                    Ok(if probes <= 3 {
                        vec!["2400:00000000".into()]
                    } else {
                        expected()
                    })
                },
                || {
                    restarts += 1;
                    Ok(())
                },
                || {}
            )
            .unwrap()
        );
        assert_eq!(restarts, 1);
        assert_eq!(probes, 4);
    }
    #[test]
    fn delayed_delivery_and_check_only_never_restart() {
        let mut probes = 0;
        assert!(
            !ensure_profiles(
                &expected(),
                false,
                || {
                    probes += 1;
                    Ok(if probes == 1 {
                        vec!["2400:00000000".into()]
                    } else {
                        expected()
                    })
                },
                || panic!(),
                || {}
            )
            .unwrap()
        );
        assert!(
            ensure_profiles(
                &expected(),
                true,
                || Ok(vec!["2400:00000000".into()]),
                || panic!(),
                || {}
            )
            .is_err()
        );
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
        assert!(!ensure_profiles(&[], false, || panic!(), || panic!(), || {}).unwrap());
    }
    #[test]
    fn recovery_failure_and_persistently_stale_profiles_fail() {
        assert!(
            ensure_profiles(
                &expected(),
                false,
                || Ok(vec!["2400:00000000".into()]),
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
                || Ok(vec!["2400:00000000".into()]),
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
}
