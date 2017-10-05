#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InputList {
    __inner: Vec<InputListItem>
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InputListItem {
    __inner: String
}

impl From<String> for InputListItem {
    fn from(string: String) -> InputListItem {
        InputListItem { __inner: string }
    }
}

impl From<String> for InputList {
    fn from(string: String) -> InputList {
        InputList {
            __inner: string.split(";")
                .map(|s| InputListItem { __inner: s.to_owned() })
                .collect()
        }
    }
}

impl From<Vec<String>> for InputList {
    fn from(str_vec: Vec<String>) -> InputList {
        InputList {
            __inner: str_vec.into_iter()
                .map(|s| InputListItem {
                    __inner: s
                })
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
            .map(|i| i.__inner)
            .collect();
        x.join(";")
    }
}

impl InputList {
    pub fn inner_mut(&mut self) -> &mut Vec<InputListItem> {
        &mut self.__inner
    }

    pub fn inner(&self) -> &Vec<InputListItem> {
        &self.__inner
    }

    pub fn into_inner(mut self) -> Vec<InputListItem> {
        self.__inner
    }
}

impl InputListItem {
    pub fn lcid(&self) -> &str {
        &self.__inner[0..4]
    }

    pub fn kbid(&self) -> &str {
        &self.__inner[5..13]
    }

    pub fn inner(&self) -> &str {
        &self.__inner
    }
}