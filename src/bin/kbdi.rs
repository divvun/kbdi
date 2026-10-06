use clap::Parser;
use kbdi::*;

#[derive(Debug, Parser)]
#[command(about = "Configure Windows registry values for keyboards")]
enum Opt {
    #[command(
        name = "keyboard_install",
        about = "Installs a keyboard layout to the registry"
    )]
    KeyboardInstall {
        /// Language tag in BCP 47 format (eg: sma-Latn-NO)
        #[arg(short, long)]
        tag: String,
        /// Layout name (eg: Skolt Sami (Norway))
        #[arg(short = 'n', long)]
        layout: String,
        /// Product code GUID (eg: {42c3de12-28...})
        #[arg(short, long)]
        guid: String,
        /// Name of keyboard DLL (eg: kbdfoo01.dll)
        #[arg(short, long)]
        dll: String,
        /// Native language name, if required (eg: Norsk)
        #[arg(short, long)]
        lang: Option<String>,
        /// Enable keyboard immediately after installing
        #[arg(short, long)]
        enable: bool,
        /// Batch installation: verify once with keyboard_refresh after all layouts
        #[arg(long, requires = "enable")]
        defer_refresh: bool,
    },
    #[command(
        name = "keyboard_uninstall",
        about = "Uninstalls a keyboard layout from the registry"
    )]
    KeyboardUninstall {
        /// Product code GUID (eg: {42c3de12-28...})
        guid: String,
    },
    #[command(name = "keyboard_enable", about = "Enables a keyboard for a user")]
    KeyboardEnable {
        /// Language tag in BCP 47 format (eg: sma-Latn-NO)
        #[arg(short, long)]
        tag: String,
        /// Product code GUID (eg: {42c3de12-28...})
        #[arg(short, long)]
        guid: String,
        /// Native language name, if required (eg: Norsk)
        #[arg(short, long)]
        lang: Option<String>,
        /// Offer the keyboard's layout on the welcome screen instead of
        /// enabling it for this user (requires admin)
        #[arg(short, long)]
        default_user: bool,
        /// Batch activation: verify once with keyboard_refresh after all layouts
        #[arg(long)]
        defer_refresh: bool,
    },
    #[cfg(not(feature = "legacy"))]
    #[command(
        name = "keyboard_refresh",
        about = "Verify live keyboard profiles and refresh stale text services in this session"
    )]
    KeyboardRefresh {
        /// Check only; do not restart text services
        #[arg(long)]
        check: bool,
    },
    #[cfg(not(feature = "legacy"))]
    #[command(name = "__keyboard_profiles", hide = true)]
    KeyboardProfiles,
    #[command(name = "registry_regen", about = "Enable a language with provided tag")]
    RegistryRegen,
    #[command(
        name = "language_enable",
        about = "Enable a language with provided tag"
    )]
    LanguageEnable {
        /// Language tag in BCP 47 format (eg: sma-Latn-NO)
        tag: String,
    },
    #[command(name = "language_query", about = "Get data about language tag")]
    LanguageQuery {
        /// Language tag in BCP 47 format (eg: sma-Latn-NO)
        tag: String,
    },
    #[command(
        name = "language_list",
        about = "Lists all languages enabled for the current user"
    )]
    LanguageList,
    #[command(
        name = "keyboard_list",
        about = "Lists all enabled keyboards for the user"
    )]
    KeyboardList,
    #[command(
        name = "keyboard_enabled",
        about = "Lists all enabled keyboards for the user"
    )]
    KeyboardEnabled,
    #[command(about = "Remove empty languages and invalid keyboards")]
    Clean,
}

fn main() {
    let opt = Opt::parse();
    #[cfg(not(feature = "legacy"))]
    if matches!(opt, Opt::KeyboardProfiles) {
        if let Err(error) = text_services::probe() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    kbdi::setup_logger().unwrap_or_else(|_| eprintln!("Logger failed to init."));
    if let Err(error) = run(opt) {
        eprintln!("kbdi: {error}");
        std::process::exit(1);
    }
}

fn run(opt: Opt) -> Result<(), Box<dyn std::error::Error>> {
    match opt {
        Opt::KeyboardInstall {
            tag,
            layout,
            guid,
            dll,
            lang,
            enable,
            defer_refresh,
        } => {
            match keyboard::install(&tag, &layout, &guid, &dll, lang.as_deref()) {
                Ok(()) | Err(keyboard::Error::AlreadyExists) => (),
                Err(error) => return Err(error.into()),
            }
            if enable {
                keyboard::enable(&tag, &guid, lang.as_deref())?;
                finish_enable(defer_refresh)?;
            }
        }
        Opt::KeyboardUninstall { guid } => keyboard::uninstall(&guid)?,
        Opt::KeyboardEnable {
            tag,
            guid,
            lang,
            default_user,
            defer_refresh,
        } => {
            if default_user {
                #[cfg(not(feature = "legacy"))]
                return Ok(keyboard::enable_default_user(&guid)?);
                #[cfg(feature = "legacy")]
                {
                    eprintln!("--default-user is not implemented; no keyboard changes were made");
                    std::process::exit(2);
                }
            }
            keyboard::enable(&tag, &guid, lang.as_deref())?;
            finish_enable(defer_refresh)?;
        }
        #[cfg(not(feature = "legacy"))]
        Opt::KeyboardRefresh { check } => text_services::refresh(check)?,
        #[cfg(not(feature = "legacy"))]
        Opt::KeyboardProfiles => unreachable!("probe handled before logger initialization"),
        Opt::RegistryRegen => keyboard::regenerate_registry(),
        Opt::LanguageEnable { tag } => enable_language(&tag)?,
        Opt::LanguageQuery { tag } => println!("{}", query_language(&tag)),
        Opt::LanguageList => println!("{}", enabled_languages()?.join(" ")),
        Opt::KeyboardList => {
            for keyboard in keyboard::installed() {
                println!("{keyboard}");
            }
        }
        Opt::KeyboardEnabled => {
            for keyboard in enabled_keyboards()? {
                println!("{keyboard:?}");
            }
        }
        Opt::Clean => clean().map_err(std::io::Error::other)?,
    }
    Ok(())
}

fn finish_enable(defer_refresh: bool) -> Result<(), Box<dyn std::error::Error>> {
    if defer_refresh {
        log::info!(
            "Keyboard preference saved; live verification deferred. Run keyboard_refresh after all layouts are registered and enabled."
        );
        return Ok(());
    }
    #[cfg(not(feature = "legacy"))]
    text_services::refresh(false).map_err(|error| {
        std::io::Error::other(format!(
            "keyboard preference saved, but live activation was not verified: {error}"
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn parser_definition_is_valid_and_has_no_author() {
        Opt::command().debug_assert();
        assert!(Opt::command().get_author().is_none());
        assert_eq!(
            Opt::try_parse_from(["kbdi", "--help"]).unwrap_err().kind(),
            clap::error::ErrorKind::DisplayHelp
        );
    }

    #[test]
    fn installer_command_names_and_flags_are_preserved() {
        let parsed = Opt::try_parse_from([
            "kbdi",
            "keyboard_install",
            "-t",
            "sma-Latn-NO",
            "-n",
            "Sámi keyboard",
            "-g",
            "{guid}",
            "-d",
            "kbdfoo.dll",
            "-l",
            "Sámi",
            "-e",
        ])
        .unwrap();
        let Opt::KeyboardInstall {
            tag,
            layout,
            guid,
            dll,
            lang,
            enable,
            defer_refresh,
        } = parsed
        else {
            panic!("wrong command");
        };
        assert!(!defer_refresh);
        assert_eq!(
            (
                tag.as_str(),
                layout.as_str(),
                guid.as_str(),
                dll.as_str(),
                lang.as_deref(),
                enable
            ),
            (
                "sma-Latn-NO",
                "Sámi keyboard",
                "{guid}",
                "kbdfoo.dll",
                Some("Sámi"),
                true
            )
        );
        assert!(matches!(
            Opt::try_parse_from(["kbdi", "keyboard_uninstall", "{guid}"]).unwrap(),
            Opt::KeyboardUninstall { .. }
        ));
        assert!(matches!(
            Opt::try_parse_from([
                "kbdi",
                "keyboard_enable",
                "--tag",
                "se",
                "--guid",
                "{guid}",
                "--default-user"
            ])
            .unwrap(),
            Opt::KeyboardEnable {
                default_user: true,
                ..
            }
        ));
        for command in [
            "registry_regen",
            "language_list",
            "keyboard_list",
            "keyboard_enabled",
            "clean",
        ] {
            Opt::try_parse_from(["kbdi", command]).unwrap();
        }
        for command in ["language_enable", "language_query"] {
            Opt::try_parse_from(["kbdi", command, "se"]).unwrap();
        }
    }

    #[test]
    fn batch_requires_enable_and_exposes_final_verification() {
        let args = [
            "kbdi",
            "keyboard_install",
            "-t",
            "sjd-Cyrl",
            "-g",
            "{guid}",
            "-n",
            "Test",
            "-d",
            "kbdtest.dll",
            "--defer-refresh",
        ];
        assert!(Opt::try_parse_from(args).is_err());
        assert!(Opt::try_parse_from(args.into_iter().chain(["-e"])).is_ok());
        #[cfg(not(feature = "legacy"))]
        assert!(matches!(
            Opt::try_parse_from(["kbdi", "keyboard_refresh", "--check"]).unwrap(),
            Opt::KeyboardRefresh { check: true }
        ));
    }

    #[test]
    fn incomplete_or_unknown_commands_are_rejected() {
        for args in [
            vec!["kbdi"],
            vec!["kbdi", "keyboard_install", "-t", "se"],
            vec!["kbdi", "keyboard_enable", "--unknown"],
        ] {
            assert!(Opt::try_parse_from(args).is_err());
        }
    }
}
