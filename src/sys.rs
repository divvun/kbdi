#![cfg(windows)]
extern crate winapi;

pub mod bcp47langs {
    use winapi::winrt::hstring::HSTRING;
    use winapi::ctypes::{c_char, c_int};
    
    #[link(name = "BCP47Langs")]
    extern "system" {
        pub fn GetUserLanguages(delimiter: c_char, string: *mut HSTRING) -> c_int;
        pub fn LcidFromBcp47(tag: HSTRING, lcid: *mut c_int) -> c_int;
    }
}

pub mod input {
    use winapi::ctypes::*;
    use winapi::um::winnt::WCHAR;
    
    #[link(name = "input")]
    extern "system" {
        pub fn InstallLayoutOrTip(tip_string: *const WCHAR, flags: c_int) -> c_int;
        //pub fn SetDefaultLayoutOrTip(tip_string: *const WCHAR, flags: c_int) -> c_int;
    }
}

pub mod winlangdb {
    use winapi::ctypes::*;
    use winapi::um::winnt::WCHAR;
    use winapi::winrt::hstring::HSTRING;
    
    #[link(name = "winlangdb")]
    extern "system" {
        pub fn EnsureLanguageProfileExists() -> c_int;
        pub fn GetLanguageNames(language: *const WCHAR, autonym: *mut WCHAR, english_name: *mut WCHAR, local_name: *mut WCHAR, script_name: *mut WCHAR) -> c_int;
        pub fn RemoveInputsForAllLanguagesInternal() -> c_int;
        pub fn SetUserLanguages(delimiter: c_char, user_languages: HSTRING) -> c_int;
        pub fn GetDefaultInputMethodForLanguage(language: HSTRING, tip_string: *mut HSTRING) -> c_int;
        pub fn TransformInputMethodsForLanguage(tip_string: HSTRING, tag: HSTRING, transformed_tip_string: *mut HSTRING) -> c_int;
    }
}

// extern "system" {
//     [DllImport("kernelbase.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int NlsUpdateLocale(string LocaleName, int Flags);

//     [DllImport("intl.cpl", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int IntlUpdateSystemLocale(string LocaleName, int dwFlags);

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int GetUserLanguages(char Delimiter, [MarshalAs(UnmanagedType.HString)] ref string UserLanguages);

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int GetUserLanguageInputMethods(string Language, char Delimiter, [MarshalAs(UnmanagedType.HString)] ref string InputMethods);

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int LcidFromBcp47([MarshalAs(UnmanagedType.HString)] string LanguageTag, ref int Lcid);

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int GetUserDisplayLanguageOverride([MarshalAs(UnmanagedType.HString)] ref string language);

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int SetUserDisplayLanguageOverride(string LanguageTag);

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int ClearUserDisplayLanguageOverride();

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int GetHttpAcceptLanguageOptOut(ref bool IsOptOut);

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int SetHttpAcceptLanguageOptOut();

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int ClearHttpAcceptLanguageOptOut();

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int GetUserLocaleFromLanguageProfileOptOut(ref bool IsOptOut);

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int SetUserLocaleFromLanguageProfileOptOut();

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int ClearUserLocaleFromLanguageProfileOptOut();

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int RemoveInputsForAllLanguagesInternal();

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int SetInputMethodOverride(string TipString);

//     [DllImport("bcp47langs.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int Bcp47GetIsoLanguageCode([MarshalAs(UnmanagedType.HString)] string languageTag, [MarshalAs(UnmanagedType.HString)] ref string isoLanguageCode);

//     [DllImport("winlangdb.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int GetDefaultInputMethodForLanguage([MarshalAs(UnmanagedType.HString)] string Language, [MarshalAs(UnmanagedType.HString)] ref string DefaultTipString);

//     [DllImport("winlangdb.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int TransformInputMethodsForLanguage([MarshalAs(UnmanagedType.HString)] string TipString, [MarshalAs(UnmanagedType.HString)] string Language, [MarshalAs(UnmanagedType.HString)] ref string TransformedTipString);

//     [DllImport("winlangdb.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int SetUserLanguages(char Delimiter, [MarshalAs(UnmanagedType.HString)] string UserLanguages);

//     [DllImport("winlangdb.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int GetLanguageNames(string Language, StringBuilder Autonym, StringBuilder EnglishName, StringBuilder LocalName, StringBuilder ScriptName);

//     [DllImport("winlangdb.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int IsImeInputMethod([MarshalAs(UnmanagedType.HString)] string TipString, ref int result);

//     [DllImport("winlangdb.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int EnsureLanguageProfileExists();
 
//     [DllImport("input.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int InstallLayoutOrTip(string TipString, int Flags);

//     [DllImport("input.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int SetDefaultLayoutOrTip(string TipString, int Flags);

//     [DllImport("input.dll", CharSet = CharSet.Unicode, SetLastError = true)]
//     public static extern int GetLayoutDescription(string LayoutId, StringBuilder LayoutDescription, ref int DescriptionLength);
// }