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
    },
    #[structopt(name = "language_query", about = "Get data about language tag")]
    LanguageQuery {
        /// Language tag in BCP 47 format (eg: sma-Latn-NO)
        tag: String,
    },
    #[structopt(
        name = "keyboard_list",
        about = "Lists all keyboards installed on the system"
    )]
    KeyboardList,
    #[structopt(about = "Remove empty languages and invalid keyboards")]
    Clean,
}

fn main() {
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
            println!("Installing keyboard...");
            match keyboard::install(&tag, &layout, &guid, &dll, lang.as_deref()) {
                Ok(_) => (),
                Err(err) => match err {
                    keyboard::Error::AlreadyExists => {
                        println!("Keyboard already installed.");
                    }
                    _ => panic!(err),
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
