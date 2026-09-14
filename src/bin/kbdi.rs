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
        /// Enable keyboard for the default user (requires admin)
        #[arg(short, long)]
        default_user: bool,
    },
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
        } => {
            match keyboard::install(&tag, &layout, &guid, &dll, lang.as_deref()) {
                Ok(()) | Err(keyboard::Error::AlreadyExists) => (),
                Err(error) => return Err(error.into()),
            }
            if enable {
                keyboard::enable(&tag, &guid, lang.as_deref())?;
            }
        }
        Opt::KeyboardUninstall { guid } => keyboard::uninstall(&guid)?,
        Opt::KeyboardEnable {
            tag,
            guid,
            lang,
            default_user,
        } => {
            if default_user {
                eprintln!("--default-user is not implemented; no keyboard changes were made");
                std::process::exit(2);
            }
            keyboard::enable(&tag, &guid, lang.as_deref())?;
        }
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
        } = parsed
        else {
            panic!("wrong command");
        };
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
