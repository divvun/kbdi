#[cfg(not(feature = "legacy"))]
pub mod bcp47langs;
#[cfg(not(feature = "legacy"))]
pub mod winlangdb;
pub mod winnls;
pub mod sys;

pub mod input {
    use super::*;
    use std::io;
    use crate::types::InputList;
    use std::ptr::null;
    use crate::winrust::to_wide_string;

    pub fn install_layout(inputs: InputList, flag: i32) -> Result<(), io::Error> {
        let input_string = String::from(inputs);
        let winput = to_wide_string(&input_string);

        let ret = unsafe { sys::input::InstallLayoutOrTipUserReg(null(), null(), null(), winput.as_ptr(), flag) };
        if ret < 0 {
            return Err(io::Error::last_os_error())
        }

        Ok(())
    }
}

pub mod winuser {
    use winapi::um::winuser;
    use crate::winrust::to_wide_string;

    pub fn load_keyboard_layout(klid: &str) {
        unsafe { winuser::LoadKeyboardLayoutW(to_wide_string(klid).as_ptr(), 
            winuser::KLF_ACTIVATE | winuser::KLF_SETFORPROCESS) };
    }
}