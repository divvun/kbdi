use kbdi::*;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(
    about = "Configure Windows registry values for keyboards",
    author = "Brendan Molloy <brendan@bbqsrc.net>"
)]
enum Opt {
    #[structopt(
        name = "keyboard_install",
        about = "Installs a keyboard layout to the registry"
    )]
    KeyboardInstall {
        /// Language tag in BCP 47 format (eg: sma-Latn-NO)
        #[structopt(short, long)]
        tag: String,
        /// Layout name (eg: Skolt Sami (Norway))
        #[structopt(short = "n", long)]
        layout: String,
        /// Product code GUID (eg: {42c3de12-28...})
        #[structopt(short, long)]
        guid: String,
        /// Name of keyboard DLL (eg: kbdfoo01.dll)
        #[structopt(short, long)]
        dll: String,
        /// Native language name, if required (eg: Norsk)
        #[structopt(short, long)]
        lang: Option<String>,
        /// Enable keyboard immediately after installing
        #[structopt(short, long)]
        enable: bool,
    },
    #[structopt(
        name = "keyboard_uninstall",
        about = "Uninstalls a keyboard layout from the registry"
    )]
    KeyboardUninstall {
        /// Product code GUID (eg: {42c3de12-28...})
        guid: String,
    },
    #[structopt(name = "keyboard_enable", about = "Enables a keyboard for a user")]
    KeyboardEnable {
        /// Language tag in BCP 47 format (eg: sma-Latn-NO)
        #[structopt(short, long)]
        tag: String,
        /// Product code GUID (eg: {42c3de12-28...})
        #[structopt(short, long)]
        guid: String,
        /// Native language name, if required (eg: Norsk)
        #[structopt(short, long)]
        lang: Option<String>,
        /// Enable keyboard for the default user (requires admin)
        #[structopt(short, long)]
        default_user: bool,
    },
    #[structopt(name = "registry_regen", about = "Enable a language with provided tag")]
    RegistryRegen,
    #[structopt(
        name = "language_enable",
        about = "Enable a language with provided tag"
    )]
    LanguageEnable {
        /// Language tag in BCP 47 format (eg: sma-Latn-NO)
        tag: String,
    },
    #[structopt(name = "language_query", about = "Get data about language tag")]
    LanguageQuery {
        /// Language tag in BCP 47 format (eg: sma-Latn-NO)
        tag: String,
    },
    #[structopt(
        name = "language_list",
        about = "Lists all languages enabled for the current user"
    )]
    LanguageList,
    #[structopt(
        name = "keyboard_list",
        about = "Lists all enabled keyboards for the user"
    )]
    KeyboardList,
    #[structopt(
        name = "keyboard_enabled",
        about = "Lists all enabled keyboards for the user"
    )]
    KeyboardEnabled,
    #[structopt(about = "Remove empty languages and invalid keyboards")]
    Clean,
}

// This code really does not work as a 64bit binary due to bugs in Windows.
#[cfg(target_pointer_width = "32")]
fn main() {
    kbdi::setup_logger().unwrap_or_else(|_| eprintln!("Logger failed to init."));
    log::info!("Starting Divvun Keyboard Installer...");

    let _guard = option_env!("SENTRY_DSN").map(|var| sentry::init(var));
    let opt = Opt::from_args();

    match opt {
        Opt::KeyboardInstall {
            tag,
            layout,
            guid,
            dll,
            lang,
            enable,
        } => {
            log::info!("Installing keyboard...");
            match keyboard::install(&tag, &layout, &guid, &dll, lang.as_deref()) {
                Ok(_) => (),
                Err(err) => match err {
                    keyboard::Error::AlreadyExists => {
                        log::info!("Keyboard already installed.");
                    }
                    _ => panic!(err),
                },
            }
            if enable {
                log::info!("Enabling keyboard...");
                keyboard::enable(&tag, &guid, lang.as_deref()).unwrap();
            }
        }
        Opt::KeyboardUninstall { guid } => {
            keyboard::uninstall(&guid).unwrap();
        }
        Opt::KeyboardEnable {
            tag,
            guid,
            lang,
            default_user,
        } => {
            keyboard::enable(&tag, &guid, lang.as_deref()).unwrap();
        }
        Opt::RegistryRegen => {
            keyboard::regenerate_registry();
        }
        Opt::LanguageEnable { tag } => {
            enable_language(&tag).unwrap();
        }
        Opt::LanguageQuery { tag } => {
            println!("{}", query_language(&tag));
        }
        Opt::LanguageList => {
            let languages = enabled_languages().unwrap().join(" ");
            println!("{}", &languages);
        }
        Opt::KeyboardList => {
            for k in keyboard::installed().iter() {
                println!("{}", k);
            }
        }
        Opt::KeyboardEnabled => {
            for k in enabled_keyboards().iter() {
                println!("{:?}", k);
            }
        }
        Opt::Clean => {
            clean().unwrap();
        }
    }
}
