use platform::*;
use types::*;
use keyboard::{Error, KeyboardRegKey};
use language::LanguageRegKey;

fn enabled_input_methods() -> InputList {
    let langs = ::enabled_languages().unwrap();
    let mut imes: Vec<String> = vec![];
    for lang in langs {
        imes.append(&mut bcp47langs::get_user_language_input_methods(&lang)
                .unwrap());
    }
    InputList::from(imes)
}

pub fn enable(tag: &str, product_code: &str, lang_name: Option<&str>) -> Result<(), Error> {
    let record = match KeyboardRegKey::find_by_product_code(product_code) {
        Some(v) => v,
        None => return Err(Error::NotFound)
    };

    // Check language is enabled or LCID check will fail
    println!("D: Enabling language by tag");
    ::enable_language(tag).unwrap();

    // Set human visible language in dropdown
    if let Some(lang) = lang_name {
         println!("D: Setting cached language name");

         if let Some(mut lrk) = LanguageRegKey::find_by_tag(tag) {
             lrk.set_language_name(lang);
         }
    }
    
    // Generate input list item
    println!("D: Get LCID from tag");
    let lcid = bcp47langs::lcid_from_bcp47(tag).unwrap();
    let tip = InputList::from(format!("{:04X}:{}", lcid, record.regkey_id()));

    println!("D: Install layout, flag 0");
    input::install_layout(tip, 0).unwrap();

    Ok(())
}

#[cfg(not(feature = "legacy"))]
pub fn remove_invalid_kbids() {
    let installed_imes: Vec<String> = KeyboardRegKey::installed().iter()
        .map(|x| x.regkey_id().to_owned())
        .collect();

    let enabled_imes = enabled_input_methods();
    let filtered_imes: Vec<InputListItem> = enabled_imes.into_inner()
        .into_iter()
        .filter(|i| {
            let kbid = i.kbid().to_string().to_lowercase();
            // Only handle custom keyboards
            if kbid.starts_with("a") {
                return true;
            }
            installed_imes.contains(&kbid)
        })
        .collect();

    bcp47langs::remove_inputs_for_all_languages().unwrap();
    input::install_layout(InputList::from(filtered_imes), 0).unwrap();
}