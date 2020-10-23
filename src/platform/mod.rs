#[cfg(not(feature = "legacy"))]
mod win8;
#[cfg(feature = "legacy")]
mod win7;
pub mod winnls;
pub mod sys;

#[cfg(not(feature = "legacy"))]
pub use win8::keyboard as keyboard;
#[cfg(feature = "legacy")]
pub use win7::keyboard as keyboard;

#[cfg(not(feature = "legacy"))]
pub use win8::clean;
#[cfg(feature = "legacy")]
pub use win7::clean;

pub mod input {
    use super::*;
    use std::io;
    use crate::types::InputList;
    use std::ptr::null;
    use crate::winrust::to_wide_string;

    pub fn install_layout(inputs: InputList, flag: i32) -> Result<(), io::Error> {
        info!("Input list: {:?}", &inputs);
        let input_string = String::from(inputs);
        info!("Input string: {}", &input_string);
        let winput = to_wide_string(&input_string);

        // let ret = unsafe { sys::input::InstallLayoutOrTipUserReg(null(), null(), null(), winput.as_ptr(), flag) };
        let ret = unsafe { sys::input::InstallLayoutOrTip(winput.as_ptr(), flag) };
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