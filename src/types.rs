#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InputList {
    __inner: Vec<InputListItem>
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InputListItem {
    lang_id: u16,
    tip_id: u32
}

use std::u16;
use std::u32;

impl InputListItem {
    fn try_from(string: &str) -> Result<InputListItem, &'static str> {
        debug!("InputListItem try_from: {}", &string);

        
        let lang_id = u16::from_str_radix(&string[0..4], 16).unwrap();
        let tip_id = u32::from_str_radix(&string[5..13], 16).unwrap();

        Ok(InputListItem {
            lang_id,
            tip_id
        })
    }
}

impl From<String> for InputList {
    fn from(string: String) -> InputList {
        InputList {
            __inner: string.split(";")
                .map(|s| InputListItem::try_from(s).unwrap())
                .collect()
        }
    }
}

impl From<Vec<String>> for InputList {
    fn from(str_vec: Vec<String>) -> InputList {
        InputList {
            __inner: str_vec.into_iter()
                .map(|s| InputListItem::try_from(&s).unwrap())
                .collect()
        }
    }
}

impl From<Vec<InputListItem>> for InputList {
    fn from(vec: Vec<InputListItem>) -> InputList {
        InputList {
            __inner: vec
        }
    }
}

impl From<InputList> for String {
    fn from(input_list: InputList) -> String {
        let x: Vec<String> = input_list.__inner
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