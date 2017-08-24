#[macro_use]
extern crate clap;
extern crate kbdi;

use kbdi::*;

fn main() {
    let matches = clap_app!(kbdi =>
        (@setting SubcommandRequiredElseHelp)
        (version: crate_version!())
        (author: "Brendan Molloy <brendan@bbqsrc.net>")
        (about: "Configure Windows registry values for keyboards")
        (@subcommand install =>
            (about: "Installs a keyboard layout to the registry")
            (@arg GUID: -g --guid +takes_value +required "Product code GUID for linking to MSI (eg: {42c3de12-28...})")
            (@arg LANG: -l --language +takes_value +required "Native language name (eg: Norsk)")
            (@arg DLL: -d --dll +takes_value +required "Name of keyboard DLL (eg: kbdfoo01.dll)")
            (@arg LAYOUT: -n --("layout-name") +takes_value "Layout name, defaults to LANG (eg: Skolt Sami (Norway))")
            (@arg CODE: -i --("language-code") +takes_value +required "Language code in BCP 47 style (eg: sma-Latn-NO)")
        )
        (@subcommand enable_language =>
            (about: "Enable a language with provided native name and code")
            (@arg LANG: -l --language +takes_value +required "Native language name (eg: Norsk)")
            (@arg CODE: -i --("language-code") +takes_value +required "Language code in BCP 47 style (eg: sma-Latn-NO)")
        )
        (@subcommand list_languages => 
            (about: "Lists all languages enabled for the current user")
        )
        (@subcommand list_locales =>
            (about: "Lists all supported locales of the operating system")
        )
    ).get_matches();

    match matches.subcommand() {
        ("install", Some(matches)) => {
            let lang_name = matches.value_of("LANG").unwrap();
            let layout_dll = matches.value_of("DLL").unwrap();
            let layout_name = matches.value_of("LAYOUT").unwrap_or(&lang_name);
            let lang_code = matches.value_of("CODE").unwrap();
            let guid = matches.value_of("GUID").unwrap();

            install_keyboard(lang_name, guid, layout_dll, layout_name, lang_code);
            println!("Rebooting is required to complete installation.");
        },
        ("enable_language", Some(matches)) => {
            let lang_name = matches.value_of("LANG").unwrap();
            let lang_code = matches.value_of("CODE").unwrap();

            LanguageRegKey::create(lang_code, lang_name);
            enable_language(lang_code);
        },
        ("list_languages", _) => {
            let languages = enabled_languages();
            let a: Vec<&str> = languages.split("\n").collect();
            let out: String = a.join(" ");

            println!("{}", &out);
        },
        ("list_locales", _) => {
            let mut locales = system_locales();

            locales.sort();
            for locale in locales {
                println!("{}", locale);
            }
        },
        _ => {}
    }
}