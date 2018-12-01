#[macro_use] extern crate log;

mod types;
mod winrust;
mod language;
pub mod keyboard;
#[cfg(not(feature = "legacy"))]
mod keyboard_win8;
#[cfg(feature = "legacy")]
mod keyboard_legacy;
pub mod platform;

#[cfg(not(feature = "legacy"))]
mod win8;
#[cfg(not(feature = "legacy"))]
pub use self::win8::*;

#[cfg(feature = "legacy")]
mod win7;
#[cfg(feature = "legacy")]
pub use self::win7::*;

pub fn lcid(tag: &str) -> u32 {
    crate::platform::winnls::locale_name_to_lcid(&tag)
        .map(|x| if x == 0x1000 { 0x2000 } else { x })
        .unwrap_or(0x2000)
}
// #[test]
// fn test_sub_id() {
//     info!("sub_id: {:08x}", next_substitute_id(0xabcd));
//     info!("sub_id: {:08x}", next_substitute_id(0x0c09));
// }

// #[test]
// fn test_it_doth_work() {
//     let v = LanguageRegKey::next_transient_lang_id();

//     info!("Transient id: {:04x}", v);
// }

// #[test]
// fn test_lcid_from_bcp47() {
//     assert_eq!(bcp47langs::lcid_from_bcp47("en-AU"), Some(0x0c09), "en-AU");
//     assert_eq!(bcp47langs::lcid_from_bcp47("vro-Latn"), Some(0x2000), "vro-Latn");
//     assert_eq!(bcp47langs::lcid_from_bcp47("sjd-Cyrl"), Some(0x1000), "sjd-Cyrl");
// }

// #[test]
// fn test_default_input() {
//     assert_eq!(winlangdb::default_input_method("en-AU"), InputList("0C09:00000409".to_owned()))
// }

// #[test]
// fn test_transform_input() {
//     assert_eq!(
//         winlangdb::transform_input_methods(InputList("0C09:00000409".to_owned()), "en-AU"),
//         InputList("0C09:00000409".to_owned())
//     );
// }