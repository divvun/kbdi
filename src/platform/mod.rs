pub mod bcp47langs;
pub mod winlangdb;
pub mod winnls;
pub mod sys;

pub mod input {
    use ::*;
    use std::io;
    use types::InputList;

    pub fn install_layout(inputs: InputList, flag: i32) -> Result<(), io::Error> {
        let input_string = String::from(inputs);
        let winput = to_wide_string(&input_string);

        let ret = unsafe { sys::input::InstallLayoutOrTip(winput.as_ptr(), flag) };
        if ret < 0 {
            return Err(io::Error::last_os_error())
        }

        Ok(())
    }
}