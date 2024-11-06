use fltk::{prelude::WindowExt, window::Window};

use windows_core::w;
use windows::Win32::{Foundation::HWND, Security::Cryptography::{self, CryptProtectData, CRYPTPROTECT_PROMPTSTRUCT, CRYPTPROTECT_PROMPT_ON_UNPROTECT, CRYPT_INTEGER_BLOB}};


pub fn dpapi_protect(handle: &Window, buf: Vec<u8>) -> Result<Vec<u8>, DpapiError> {
    let blob = CRYPT_INTEGER_BLOB {
        cbData: buf.len() as u32,
        pbData: buf.as_mut_ptr()
    };

    let mut encrypt = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    let result = unsafe {
        CryptProtectData(
            std::ptr::from_ref(&blob),
            w!("Deimos API Token"),
            None,
            None,
            None,
            0,
            std::ptr::from_mut(&mut encrypt),
        );
    };
}


#[derive(Debug, thiserror::Error)]
pub enum DpapiError {

}
