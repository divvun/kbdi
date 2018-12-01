use kbdi::*;
use clap::{clap_app, crate_version};

fn main() {
    let _guard = sentry::init(include_str!("../../dsn.txt"));

    let matches = clap_app!(kbdi =>
        (@setting SubcommandRequiredElseHelp)
        (version: crate_version!())
        (author: "Brendan Molloy <brendan@bbqsrc.net>")
        (about: "Configure Windows registry values for keyboards")
        (@subcommand keyboard_install =>
            (about: "Installs a keyboard layout to the registry")
            (@arg TAG: -t +takes_value +required "Language tag in BCP 47 format (eg: sma-Latn-NO)")
            (@arg LAYOUT: -n +takes_value +required "Layout name (eg: Skolt Sami (Norway))")
            (@arg GUID: -g +takes_value +required "Product code GUID (eg: {42c3de12-28...})")
            (@arg DLL: -d +takes_value +required "Name of keyboard DLL (eg: kbdfoo01.dll)")
            (@arg LANG: -l +takes_value "Native language name, if required (eg: Norsk)")
            (@arg enable: -e "Enable keyboard immediately after installing")
        )
        (@subcommand keyboard_uninstall =>
            (about: "Uninstalls a keyboard layout from the registry")
            (@arg GUID: +required "Product code GUID (eg: {42c3de12-28...})")
        )
        (@subcommand keyboard_enable =>
            (about: "Enables a keyboard for a user")
            (@arg TAG: -t +takes_value +required "Language tag in BCP 47 format (eg: sma-Latn-NO)")
            (@arg GUID: -g +takes_value +required "Product code GUID (eg: {42c3de12-28...})")
            (@arg LANG: -l +takes_value "Native language name, if required (eg: Norsk)")
        )
        (@subcommand language_enable =>
            (about: "Enable a language with provided tag")
            (@arg TAG: +required "Language tag in BCP 47 format (eg: sma-Latn-NO)")
        )
        (@subcommand language_query =>
            (about: "Get data about language tag")
            (@arg TAG: +required "Language tag in BCP 47 format (eg: sma-Latn-NO)")
        )
        (@subcommand language_list => 
            (about: "Lists all languages enabled for the current user")
        )
        (@subcommand keyboard_list =>
            (about: "Lists all keyboards installed on the system")
        )
        (@subcommand keyboard_enabled =>
            (about: "Lists all enabled keyboards for the user")
        )
        (@subcommand clean =>
            (about: "Remove empty languages and invalid keyboards")
        )
    ).get_matches();

    match matches.subcommand() {
        ("keyboard_install", Some(matches)) => {
            let lang_name = matches.value_of("LANG");
            let layout_dll = matches.value_of("DLL").unwrap();
            let layout_name = matches.value_of("LAYOUT").unwrap();
            let tag = matches.value_of("TAG").unwrap();
            let guid = matches.value_of("GUID").unwrap();
            let wants_enable = matches.is_present("enable");
            
            println!("Installing keyboard...");
            match keyboard::install(tag, layout_name, guid, layout_dll, lang_name) {
                Ok(_) => (),
                Err(err) => {
                    match err {
                        keyboard::Error::AlreadyExists => {
                            println!("Keyboard already installed.");
                        },
                        _ => panic!(err)
                    }
                }
            }
            if wants_enable {
                println!("Enabling keyboard...");
                keyboard::enable(tag, guid, lang_name).unwrap();
            }
        },
        ("keyboard_uninstall", Some(matches)) => {
            let guid = matches.value_of("GUID").unwrap();
            keyboard::uninstall(guid).unwrap();
        },
        ("keyboard_enable", Some(matches)) => {
            let lang_name = matches.value_of("LANG");
            let tag = matches.value_of("TAG").unwrap();
            let guid = matches.value_of("GUID").unwrap();

            keyboard::enable(tag, guid, lang_name).unwrap();
        },
        ("language_enable", Some(matches)) => {
            let tag = matches.value_of("TAG").unwrap();
            enable_language(tag).unwrap();
        },
        ("language_query", Some(matches)) => {
            let tag = matches.value_of("TAG").unwrap();
            println!("{}", query_language(tag));
        },
        ("language_list", _) => {
            let languages = enabled_languages().unwrap().join(" ");
            println!("{}", &languages);
        },
        ("keyboard_list", _) => {
            for k in keyboard::installed().iter() {
                println!("{}", k);
            }
        },
        ("keyboard_enabled", _) => {
            for k in enabled_keyboards().iter() {
                println!("{:?}", k);
            }
        },
        ("clean", _) => {
            clean().unwrap();
        },
        _ => {}
    };
}
