pub mod keyboard;

use super::winnls;

pub fn query_language(tag: &str) -> String {
    let id = winnls::resolve_locale_name(tag)
        .unwrap_or(tag.to_owned());

    let a = format!("Tag:  {}", id);

    let b = match winnls::locale_name_to_lcid(&id) {
        Ok(lcid) => format!("LCID: 0x{:08x}", lcid),
        Err(_) => format!("LCID: undefined")
    };

    format!("{}\n{}", a, b)
}

pub fn clean() -> Result<(), String> {
    crate::keyboard::remove_invalid();
    Ok(())
}
