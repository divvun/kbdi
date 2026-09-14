use windows_registry::{CURRENT_USER, Key};

#[allow(dead_code)]
pub struct LanguageRegKey {
    id: String,
    pub(crate) regkey: Key,
}

impl LanguageRegKey {
    pub fn set_language_name(&mut self, name: &str) {
        self.regkey.set_string("CachedLanguageName", name).unwrap();
    }

    pub fn find_by_tag(tag: &str) -> Option<Self> {
        CURRENT_USER
            .options()
            .read()
            .write()
            .open(format!(r"Control Panel\International\User Profile\{tag}"))
            .ok()
            .map(|regkey| Self {
                id: tag.to_owned(),
                regkey,
            })
    }
}
