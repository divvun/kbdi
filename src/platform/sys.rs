#![allow(non_snake_case)]

macro_rules! lib_extern {
    ( $($name:ident ( $($arg: ident : $argty: ty),* ) -> $retty: ty);+ ) => {
        $(pub unsafe fn $name($($arg: $argty),*) -> $retty {
            let func: Symbol<unsafe extern "stdcall" fn($($arg: $argty),*) -> $retty> =
                LIB.get(stringify!($name).as_bytes()).unwrap();
            func($($arg),*)
        })+
    };
}

pub mod input {
    use winapi::ctypes::*;
    use winapi::um::winnt::WCHAR;
    use libloading::os::windows::*;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref LIB: Library = Library::new(r"C:\Windows\System32\input.dll").unwrap();
    }

    lib_extern! {
        InstallLayoutOrTip(tip_string: *const WCHAR, flags: c_int) -> c_int;
        InstallLayoutOrTipUserReg(user_reg: *const WCHAR, system_reg: *const WCHAR, software_reg: *const WCHAR, 
            tip_string: *const WCHAR, flags: c_int) -> c_int
    }
}