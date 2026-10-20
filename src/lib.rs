#![deny(unsafe_op_in_unsafe_fn)]

#[macro_use]
extern crate log;

pub mod keyboard;
#[cfg(feature = "legacy")]
mod keyboard_legacy;
#[cfg(not(feature = "legacy"))]
mod keyboard_win8;
mod language;
pub mod platform;
mod registry_snapshot;
#[cfg(not(feature = "legacy"))]
pub mod text_services;
#[cfg(not(feature = "legacy"))]
pub mod tsf;
mod types;
#[cfg(not(feature = "legacy"))]
mod welcome_screen;
mod winrust;

#[cfg(not(feature = "legacy"))]
mod win8;
#[cfg(not(feature = "legacy"))]
pub use self::win8::*;

#[cfg(feature = "legacy")]
mod win7;
#[cfg(feature = "legacy")]
pub use self::win7::*;

// [spec:kbdgen:def:tsf.register.ids+1]
/// The LCID of `tag`'s Windows locale, if it has one. `LocaleNameToLCID`
/// gives a tag without one `LOCALE_CUSTOM_UNSPECIFIED` (`0x1000`), or, once
/// the tag is in the user's language list, a transient LCID (`0x3000` on
/// Windows 10), which is no locale's own and not the LANGID the user's
/// input list gives the tag. Both are `LANG_NEUTRAL`, as no locale is.
pub fn windows_lcid(tag: &str) -> Option<u32> {
    crate::platform::winnls::locale_name_to_lcid(tag)
        .ok()
        .filter(|&lcid| has_language(lcid))
}

/// Whether `lcid` names a language: its primary language is not
/// `LANG_NEUTRAL`.
fn has_language(lcid: u32) -> bool {
    lcid & 0x3FF != 0
}

/// The LCID whose low word names the KLIDs of `tag`: its Windows locale's,
/// or `0x2000` for a tag without one.
pub fn lcid(tag: &str) -> u32 {
    windows_lcid(tag).unwrap_or(0x2000)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("Set logger error")]
    SetLoggerError(#[from] log::SetLoggerError),
}

pub fn setup_logger() -> Result<(), Error> {
    let log_path = platform::native::log_directory()?;

    std::fs::create_dir_all(&log_path)?;

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {:<5} {}] {}",
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .level_for("kbdi", log::LevelFilter::Trace)
        .chain(std::io::stdout())
        .chain(fern::log_file(log_path.join(format!("run.log")))?)
        .apply()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_lcids_name_no_language() {
        for lcid in [0x0409, 0x0C3B, 0x143B, 0x0001_0407] {
            assert!(has_language(lcid), "{lcid:x}");
        }
        // LOCALE_CUSTOM_UNSPECIFIED, transient LCIDs, LOCALE_USER_DEFAULT.
        for lcid in [0x1000, 0x2000, 0x2400, 0x3000, 0x0400, 0] {
            assert!(!has_language(lcid), "{lcid:x}");
        }
    }
}
