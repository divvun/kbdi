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
            (@arg LANG: -l --language +takes_value +required "Language name in specified language (eg: Norsk)")
            (@arg DLL: -d --dll +takes_value +required "Name of keyboard DLL (eg: kbdfoo01.dll)")
            (@arg LAYOUT: -n --layout_name +takes_value "Layout name, defaults to LANG (eg: US Extended)")
            (@arg CODE: -i --language_code +takes_value +required "Language code as supported by Windows (eg: sma-Latn-NO)")
        )
        (@subcommand enabled_languages => 
            (about: "Lists all languages enabled for the current user from the registry")
        )
        (@subcommand list_locales =>
            (about: "Lists all supported locales of the operating system")
        )
    ).get_matches();

    match matches.subcommand_name() {
        Some("install") => {
            let lang_name = matches.value_of("LANG").unwrap();
            let layout_dll = matches.value_of("DLL").unwrap();
            let layout_name = matches.value_of("LAYOUT").unwrap_or(&lang_name);
            let lang_code = matches.value_of("CODE").unwrap();
            let guid = matches.value_of("GUID").unwrap();

            install_keyboard(lang_name, guid, layout_dll, layout_name, lang_code);
        },
        Some("enabled_languages") => {
            let languages = enabled_languages();
            let a: Vec<&str> = languages.split("\n").collect();
            let out: String = a.join(" ");

            println!("{}", &out);
        }
        Some("list_locales") => {
            let mut locales = system_locales();

            locales.sort();
            for locale in locales {
                println!("{}", locale);
            }
        },
        _ => {}
    }
}