# kbdi

```
kbdi 0.3.0
Brendan Molloy <brendan@bbqsrc.net>
Configure Windows registry values for keyboards

USAGE:
    kbdi.exe <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    clean                 Remove empty languages and invalid keyboards
    help                  Prints this message or the help of the given subcommand(s)
    keyboard_enable       Enables a keyboard for a user
    keyboard_enabled      Lists all enabled keyboards for the user
    keyboard_install      Installs a keyboard layout to the registry
    keyboard_list         Lists all keyboards installed on the system
    keyboard_uninstall    Uninstalls a keyboard layout from the registry
    language_enable       Enable a language with provided tag
    language_list         Lists all languages enabled for the current user
    language_query        Get data about language tag
```

## Building

This should always be built for the `i686-pc-windows-msvc` target. The `crt-static` flag must be applied to staticly link the C runtime.

```
cargo build --release --target i686-pc-windows-msvc --bin kbdi
cargo build --release --target i686-pc-windows-msvc --features legacy --bin kbdi-legacy
```