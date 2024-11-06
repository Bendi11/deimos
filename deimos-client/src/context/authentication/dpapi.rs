use windows_core::w;
use windows::Win32::{
    Foundation::{LocalFree, HWND},
    Security::Cryptography::{
        self, CryptProtectData, CryptUnprotectData, CRYPTPROTECT_PROMPTSTRUCT, CRYPTPROTECT_PROMPT_ON_UNPROTECT, CRYPT_INTEGER_BLOB
    }
};


//#[cfg(windows)]
pub fn protect(buf: &[u8]) -> Result<Vec<u8>, DpapiError> {
    let blob = CRYPT_INTEGER_BLOB {
        cbData: buf.len() as u32,
        pbData: unsafe { buf.as_ptr() as *mut u8 },
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
        let mut vec = Vec::with_capacity(slice.len());
        vec.copy_from_slice(slice);

        LocalFree(encrypt.pbData);
        vec
    })
}

pub fn unprotect(buf: &[u8])  -> Result<Vec<u8>, DpapiError> {
    let blob = CRYPT_INTEGER_BLOB {
        cbData: buf.len() as u8,
        pbData: unsafe { buf.as_ptr() as *mut u8 },
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
        let mut vec = Vec::with_capacity(slice.len());
        vec.copy_from_slice(slice);
        
        LocalFree(unprotect.pbData);
        vec
    })
}

#[derive(Debug, thiserror::Error)]
pub enum DpapiError {
    #[error("Windows API error: {0}")]
    Windows(#[from] windows_core::Error),
}
