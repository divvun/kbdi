use std::{convert::TryFrom, fmt::Debug};

#[derive(PartialEq, Eq, Clone)]
pub struct InputList {
    __inner: Vec<InputListItem>,
}

impl Debug for InputList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(&self.__inner).finish()
    }
}

#[derive(PartialEq, Eq, Clone)]
pub struct InputListItem {
    pub lang_id: u16,
    pub tip_id: u32,
}

impl Debug for InputListItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{{{:04x}:{:08x}}}", self.lang_id, self.tip_id))
    }
}

use std::u16;
use std::u32;

impl TryFrom<&str> for InputListItem {
    type Error = ();

    fn try_from(string: &str) -> Result<InputListItem, ()> {
        log::trace!("InputListItem try_from: {}", &string);

        let lang_id = u16::from_str_radix(&string[0..4], 16).map_err(|_| ())?;
        let tip_id = u32::from_str_radix(&string[5..13], 16).map_err(|_| ())?;

        Ok(InputListItem { lang_id, tip_id })
    }
}

impl TryFrom<String> for InputList {
    type Error = ();

    fn try_from(string: String) -> Result<InputList, ()> {
        Ok(InputList {
            __inner: string
                .split(";")
                .map(|s| InputListItem::try_from(s))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<Vec<String>> for InputList {
    type Error = ();

    fn try_from(str_vec: Vec<String>) -> Result<InputList, ()> {
        Ok(InputList {
            __inner: str_vec
                .into_iter()
                .map(|s| InputListItem::try_from(&*s))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl From<Vec<InputListItem>> for InputList {
    fn from(vec: Vec<InputListItem>) -> InputList {
        InputList { __inner: vec }
    }
}

impl From<InputList> for String {
    fn from(input_list: InputList) -> String {
        let x: Vec<String> = input_list
            .__inner
            .into_iter()
            .map(|i| String::from(i))
            .collect();
        x.join(";")
    }
}

impl From<InputListItem> for String {
    fn from(input_item: InputListItem) -> String {
        format!("0x{:04X}:{:08X}", input_item.lang_id, input_item.tip_id)
    }
}

impl InputList {
    pub fn inner_mut(&mut self) -> &mut Vec<InputListItem> {
        &mut self.__inner
    }

    pub fn inner(&self) -> &Vec<InputListItem> {
        &self.__inner
    }

    pub fn into_inner(self) -> Vec<InputListItem> {
        self.__inner
    }
}

impl InputListItem {
    pub fn lcid(&self) -> String {
        format!("{:04X}", self.lang_id)
    }

    pub fn kbid(&self) -> String {
        format!("{:08X}", self.tip_id)
    }
}
