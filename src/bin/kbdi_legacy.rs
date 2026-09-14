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
    },
    #[command(name = "language_query", about = "Get data about language tag")]
    LanguageQuery {
        /// Language tag in BCP 47 format (eg: sma-Latn-NO)
        tag: String,
    },
    #[command(
        name = "keyboard_list",
        about = "Lists all keyboards installed on the system"
    )]
    KeyboardList,
    #[command(about = "Remove empty languages and invalid keyboards")]
    Clean,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    #[test]
    fn legacy_installer_command_contract() {
        Opt::command().debug_assert();
        assert!(Opt::command().get_author().is_none());
        assert!(matches!(
            Opt::try_parse_from([
                "kbdi-legacy",
                "keyboard_install",
                "-t",
                "se",
                "-n",
                "Sámi",
                "-g",
                "{guid}",
                "-d",
                "kbdfoo.dll",
                "-e"
            ])
            .unwrap(),
            Opt::KeyboardInstall { enable: true, .. }
        ));
        assert!(matches!(
            Opt::try_parse_from(["kbdi-legacy", "keyboard_enable", "-t", "se", "-g", "{guid}"])
                .unwrap(),
            Opt::KeyboardEnable { .. }
        ));
        for command in ["keyboard_list", "clean"] {
            Opt::try_parse_from(["kbdi-legacy", command]).unwrap();
        }
        assert!(Opt::try_parse_from(["kbdi-legacy", "keyboard_install"]).is_err());
    }
}

fn main() {
    let opt = Opt::parse();

    match opt {
        Opt::KeyboardInstall {
            tag,
            layout,
            guid,
            dll,
            lang,
            enable,
        } => {
            println!("Installing keyboard...");
            match keyboard::install(&tag, &layout, &guid, &dll, lang.as_deref()) {
                Ok(_) => (),
                Err(err) => match err {
                    keyboard::Error::AlreadyExists => {
                        println!("Keyboard already installed.");
                    }
                    _ => panic!("{err:?}"),
                },
            }
            if enable {
                println!("Enabling keyboard...");
                keyboard::enable(&tag, &guid).unwrap();
            }
        }
        Opt::KeyboardUninstall { guid } => {
            keyboard::uninstall(&guid).unwrap();
        }
        Opt::KeyboardEnable { tag, guid } => {
            keyboard::enable(&tag, &guid).unwrap();
        }
        Opt::LanguageQuery { tag } => {
            println!("{}", query_language(&tag));
        }
        Opt::KeyboardList => {
            for k in keyboard::installed().iter() {
                println!("{}", k);
            }
        }
        Opt::Clean => {
            clean().unwrap();
        }
    }
}
