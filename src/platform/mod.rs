#[cfg(not(feature = "legacy"))]
pub mod bcp47langs;
pub mod sys;
#[cfg(not(feature = "legacy"))]
pub mod winlangdb;
pub mod winnls;

pub mod input {
    use super::*;
    use crate::types::InputList;
    use crate::winrust::to_wide_string;
    use std::io;
    use std::ptr::null;

    pub fn install_layout(inputs: InputList, flag: i32) -> Result<(), io::Error> {
        log::debug!("install_layout({:?}, {:?})", inputs, flag);
        log::trace!("Input list: {:?}", &inputs);
        let input_string = String::from(inputs);
        log::trace!("Input string: {}", &input_string);
        let winput = to_wide_string(&input_string);

        // let ret = unsafe { sys::input::InstallLayoutOrTipUserReg(null(), null(), null(), winput.as_ptr(), flag) };
        let ret = unsafe { sys::input::InstallLayoutOrTip(winput.as_ptr(), flag) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }
}

pub mod winuser {
    use crate::winrust::to_wide_string;
    use winapi::um::winuser;

    pub fn load_keyboard_layout(klid: &str) {
        unsafe {
            winuser::LoadKeyboardLayoutW(
                to_wide_string(klid).as_ptr(),
                winuser::KLF_ACTIVATE | winuser::KLF_SETFORPROCESS,
            )
        };
    }

    pub fn current_keyboard() -> isize {
        unsafe { winuser::GetKeyboardLayout(0) as isize }
    }

    pub fn set_active_keyboard(layout: isize) {
        unsafe {
            winuser::PostMessageW(
                winuser::HWND_BROADCAST,
                winuser::WM_INPUTLANGCHANGEREQUEST,
                0,
                layout,
            )
        };
    }
}

#[cfg(not(feature = "legacy"))]
pub mod coreglobconfig {
    use super::*;
    
    pub fn sync_language_data() {
        unsafe { sys::coreglobconfig::SyncLanguageDataToCloud() }
    }
}
