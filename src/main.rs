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
            (@arg LCID: -L --lcid +takes_value "Microsoft l12n ID (eg: 00c9)")
            (@arg GUID: -g --guid +takes_value +required "Product code GUID for linking to MSI (eg: {42c3de12-28...})")
            (@arg LANG: -l --language +takes_value +required "Language name in specified language (eg: Norsk)")
            (@arg DLL: -d --dll +takes_value +required "Name of keyboard DLL (eg: kbdfoo01.dll)")
            (@arg LAYOUTNAME: -n --layout_name +takes_value "Layout name, defaults to LANGNAME (eg: US Extended)")
            (@arg CODE: -i --language_code +takes_value +required "Language code as supported by Windows (eg: sma-Latn-NO)")
        )
    ).get_matches();

    if let Some(ref matches) = matches.subcommand_matches("install") {
        let lang_name = matches.value_of("LANG").unwrap();
        let layout_dll = matches.value_of("DLL").unwrap();
        let layout_name = matches.value_of("LAYOUTNAME").unwrap_or(&lang_name);
        let lang_code = matches.value_of("CODE").unwrap();
        let guid = matches.value_of("GUID").unwrap();

        if let Some(lcid) = matches.value_of("LCID") {
            install_keyboard(lcid, lang_name, guid, layout_dll, layout_name, lang_code);
        } else {
            install_keyboard_custom(lang_name, guid, layout_dll, layout_name, lang_code);
        }
    }
}