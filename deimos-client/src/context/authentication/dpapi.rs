use winapi::{
    um::{
        wincrypt::CRYPTOAPI_BLOB,
        dpapi::CryptProtectData,
    },
    shared::miniwindef::DWORD,
};

pub fn dpapi_protect(buf: &[u8]) -> Result<Vec<u8>, DpapiError> {
    let mut blob = CRYPTOAPI_BLOB {
        cbData: buf.len() as DWORD,
        pbData: buf.as_ptr(),
    };

    let mut blob_out = CRYPTOAPI_BLOB::default();

    let rc = unsafe {
        CryptProtectData(
            &mut block as *mut CRYPTOAPI_BLOB,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),

        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DpapiError {

}
