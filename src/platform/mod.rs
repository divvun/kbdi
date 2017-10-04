pub mod bcp47langs;
pub mod winlangdb;
pub mod winnls;
pub mod sys;

pub mod input {
    use ::*;
    use std::io;
    use types::InputList;

    pub fn install_layout(inputs: InputList) -> Result<(), io::Error> {
        let winput = to_wide_string(&inputs.0);

        let ret = unsafe { sys::input::InstallLayoutOrTip(winput.as_ptr(), 0) };
        if ret < 0 {
            return Err(io::Error::last_os_error())
        }

        Ok(())
    }
}