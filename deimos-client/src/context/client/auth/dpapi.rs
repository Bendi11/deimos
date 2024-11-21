use windows_core::w;
use windows::Win32::{
    Foundation::{LocalFree, HWND, HLOCAL},
    Security::Cryptography::{
        self, CryptProtectData, CryptUnprotectData, CRYPTPROTECT_PROMPTSTRUCT, CRYPTPROTECT_PROMPT_ON_UNPROTECT, CRYPT_INTEGER_BLOB
    }
};


pub fn protect(buf: &[u8]) -> Result<Vec<u8>, DpapiError> {
    let blob = CRYPT_INTEGER_BLOB {
        cbData: buf.len() as u32,
        pbData: buf.as_ptr() as *mut u8,
    };

    let mut encrypt = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    Ok(unsafe {
        CryptProtectData(
            std::ptr::from_ref(&blob),
            w!("Deimos API Token"),
            None,
            None,
            None,
            0,
            std::ptr::from_mut(&mut encrypt),
        )?;
        
        let slice = std::slice::from_raw_parts(encrypt.pbData, encrypt.cbData as usize);
        let vec = Vec::from(slice);

        LocalFree(HLOCAL(encrypt.pbData as *mut std::ffi::c_void));
        vec
    })
}

pub fn unprotect(buf: &[u8])  -> Result<Vec<u8>, DpapiError> {
    let blob = CRYPT_INTEGER_BLOB {
        cbData: buf.len() as u32,
        pbData: buf.as_ptr() as *mut u8,
    };

    let mut unprotect = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    Ok(unsafe {
        CryptUnprotectData(
            std::ptr::from_ref(&blob),
            None,
            None,
            None,
            None,
            0,
            std::ptr::from_mut(&mut unprotect),
        )?;

        let slice = std::slice::from_raw_parts(unprotect.pbData, unprotect.cbData as usize);
        let vec = Vec::from(slice);
        
        LocalFree(HLOCAL(unprotect.pbData as *mut std::ffi::c_void));
        vec
    })
}

#[derive(Debug, thiserror::Error)]
pub enum DpapiError {
    #[error("Windows API error: {0}")]
    Windows(#[from] windows_core::Error),
}
