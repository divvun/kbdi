#[macro_use]
extern crate log;

pub mod keyboard;
#[cfg(feature = "legacy")]
mod keyboard_legacy;
#[cfg(not(feature = "legacy"))]
mod keyboard_win8;
mod language;
pub mod platform;
mod types;
mod winrust;

#[cfg(not(feature = "legacy"))]
mod win8;
#[cfg(not(feature = "legacy"))]
pub use self::win8::*;

#[cfg(feature = "legacy")]
mod win7;
#[cfg(feature = "legacy")]
pub use self::win7::*;

pub fn lcid(tag: &str) -> u32 {
    crate::platform::winnls::locale_name_to_lcid(&tag)
        .map(|x| if x == 0x1000 { 0x2000 } else { x })
        .unwrap_or(0x2000)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("Path error")]
    Path(#[from] pathos::Error),

    #[error("Set logger error")]
    SetLoggerError(#[from] log::SetLoggerError),
}

pub fn setup_logger() -> Result<(), Error> {
    let log_path = if whoami::username() == "SYSTEM" {
        pathos::system::app_log_dir("kbdi")
    } else {
        pathos::user::app_log_dir("kbdi")?
    };

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
