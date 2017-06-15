# kbdi

```
Installs a keyboard layout to the registry

USAGE:
    kbdi.exe install [OPTIONS] --guid <GUID> --language <LANG> --dll <DLL> --language_code <CODE>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -i, --language_code <CODE>        Language code as supported by Windows (eg: sma-Latn-NO)
    -d, --dll <DLL>                   Name of keyboard DLL (eg: kbdfoo01.dll)
    -g, --guid <GUID>                 Product code GUID for linking to MSI (eg: {42c3de12-28...})
    -l, --language <LANG>             Language name in specified language (eg: Norsk)
    -n, --layout_name <LAYOUTNAME>    Layout name, defaults to LANGNAME (eg: US Extended)
    -L, --lcid <LCID>                 Microsoft l12n ID (eg: 00c9)
```