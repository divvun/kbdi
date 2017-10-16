use winreg::*;
use winreg::enums::*;

fn user_profile() -> RegKey {
    RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Control Panel\\International\\User Profile", KEY_READ | KEY_WRITE)
            .unwrap()
}

pub struct LanguageRegKey {
    id: String,
    regkey: RegKey
}

impl LanguageRegKey {
    // fn next_transient_lang_id() -> u32 {
    //     let regkey = user_profile();

    //     regkey.enum_keys()
    //         .map(|x| regkey.open_subkey(x.unwrap()))
    //         .fold(0x2000u32, |acc, x| {
    //             if let Ok(lang_id) = x.unwrap().get_value::<u32, _>("TransientLangId") {
    //                 if lang_id >= acc {
    //                     lang_id + 0x400
    //                 } else {
    //                     acc
    //                 }
    //             } else {
    //                 acc
    //             }
    //         })
    // }

    // fn next_layout_order(&self) -> u32 {
    //     self.regkey.enum_values()
    //         .fold(1u32, |acc, x| {
    //             let (k, v) = x.unwrap();

    //             if k.contains(":") {
    //                 if let Ok(vv) = u32::from_reg_value(&v) {
    //                     if vv >= acc {
    //                         return acc + 1;
    //                     }
    //                 }
    //             }

    //             acc
    //         })
    // }

    pub fn set_language_name(&mut self, name: &str) {
        self.regkey.set_value("CachedLanguageName", &name).unwrap();
    }

    // fn language_name(&self) -> Option<String> {
    //     if let Ok(value) = self.regkey.get_value("CachedLanguageName") {
    //         Some(value)
    //     } else {
    //         None
    //     }
    // }

    // fn set_transient_lang_id(&mut self, id: u32) {
    //     self.regkey.set_value("TransientLangId", &id).unwrap();
    // }

    // fn transient_lang_id(&self) -> Option<u32> {
    //     if let Ok(value) = self.regkey.get_value("TransientLangId") {
    //         Some(value)
    //     } else {
    //         None
    //     }
    // }

    // fn add_keyboard(&mut self, keyboard: KeyboardRegKey) {
    //     let lcid = if let Some(v) = self.transient_lang_id() {
    //         v
    //     } else {
    //         locale_name_to_lcid(self.id.split(r"\").last().unwrap()).unwrap()
    //     } as u16;
    //     let kbd_id = keyboard.name();

    //     // Add keyboard id to reg
    //     self.regkey.set_value(format!("{:04X}:{}", &lcid, &kbd_id.to_uppercase()), &self.next_layout_order()).unwrap();

    //     // Get sub id
    //     let sub_id = format!("{:08x}", next_substitute_id(lcid));
        
    //     // Create substitute entry
    //     kbd_layout_sub_regkey(true).set_value(&sub_id, &kbd_id).unwrap();
    //     kbd_layout_sub_regkey(false).set_value(&sub_id, &kbd_id).unwrap();

    //     // Create preload entry
    //     kbd_layout_preload_regkey(true).set_value(format!("{}", next_preload_id(true)), &sub_id).unwrap();
    //     kbd_layout_preload_regkey(false).set_value(format!("{}", next_preload_id(false)), &sub_id).unwrap();
    // }

    // pub fn create(alpha_3_code: &str, native_name: &str) -> LanguageRegKey {
    //     if let Some(lang_regkey) = LanguageRegKey::find_by_alpha_3_code(&alpha_3_code) {
    //         return lang_regkey;
    //     }

    //     let mut regkey = LanguageRegKey {
    //         id: alpha_3_code.to_owned(),
    //         regkey: RegKey::predef(HKEY_CURRENT_USER)
    //             .create_subkey_with_flags(format!(r"Control Panel\International\User Profile\{}", &alpha_3_code), KEY_READ | KEY_WRITE)
    //             .unwrap()
    //     };

    //     if !system_locales().contains(&alpha_3_code.to_owned()) {
    //         regkey.set_language_name(native_name);
    //         regkey.set_transient_lang_id(LanguageRegKey::next_transient_lang_id());
    //     }

    //     regkey
    // }

    pub fn find_by_tag(tag: &str) -> Option<LanguageRegKey> {
        let maybe_regkey = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(format!(r"Control Panel\International\User Profile\{}", &tag), KEY_READ | KEY_WRITE);

        if let Ok(regkey) = maybe_regkey {
            Some(LanguageRegKey { id: tag.to_owned(), regkey: regkey })
        } else {
            None
        }
    }
}
