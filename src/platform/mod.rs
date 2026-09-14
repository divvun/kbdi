#[cfg(not(feature = "legacy"))]
pub mod bcp47langs;
pub(crate) mod native;
pub mod sys;
#[cfg(not(feature = "legacy"))]
pub mod winlangdb;
pub mod winnls;

pub mod input {
    use super::*;
    use crate::types::InputList;
    use crate::winrust::to_wide_string;
    use std::io;

    pub const ILOT_UNINSTALL: i32 = 0x00000001;

    pub fn install_layout(inputs: InputList, flag: i32) -> Result<(), io::Error> {
        log::debug!("install_layout({:?}, {:?})", inputs, flag);
        log::trace!("Input list: {:?}", &inputs);
        let input_string = String::from(inputs);
        log::trace!("Input string: {}", &input_string);
        let winput = to_wide_string(&input_string);

        // let ret = unsafe { sys::input::InstallLayoutOrTipUserReg(null(), null(), null(), winput.as_ptr(), flag) };
        let ret = unsafe { sys::input::InstallLayoutOrTip(winput.as_ptr(), flag)? };
        check_install_result(ret)
    }

    fn check_install_result(ret: i32) -> io::Result<()> {
        if ret == 0 {
            // InstallLayoutOrTip returns BOOL, and does not promise GetLastError.
            return Err(io::Error::other("InstallLayoutOrTip failed"));
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn install_layout_uses_boolean_success_semantics() {
            assert!(check_install_result(0).is_err());
            assert!(check_install_result(1).is_ok());
            assert!(check_install_result(-1).is_ok());
        }
    }
}

pub mod winuser {
    use crate::winrust::to_wide_string;
    use windows_sys::Win32::UI::{
        Input::KeyboardAndMouse as keyboard, WindowsAndMessaging as winuser,
    };

    pub fn load_keyboard_layout(klid: &str) {
        unsafe {
            keyboard::LoadKeyboardLayoutW(
                to_wide_string(klid).as_ptr(),
                keyboard::KLF_ACTIVATE | keyboard::KLF_SETFORPROCESS,
            )
        };
    }

    pub fn current_keyboard() -> isize {
        unsafe { keyboard::GetKeyboardLayout(0) as isize }
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
        // Synchronization is best effort; unavailable APIs must not panic.
        if let Err(error) = unsafe { sys::coreglobconfig::SyncLanguageDataToCloud() } {
            log::warn!("Language synchronization unavailable: {error}");
        }
    }
}
