//! Stream keys live in the current user's Windows Credential Manager, never in profile TOML.
use std::io;
use windows_sys::Win32::{Foundation::ERROR_NOT_FOUND, Security::Credentials::*};

fn target(reference: &str) -> io::Result<Vec<u16>> {
    if !reference.starts_with("CaptureStudio/stream/") || reference.contains('\0') || reference.len() > 512 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid stream credential reference"));
    }
    Ok(reference.encode_utf16().chain(Some(0)).collect())
}
pub fn new_reference() -> String {
    format!("CaptureStudio/stream/{}", uuid::Uuid::new_v4())
}

pub fn load(reference: &str) -> io::Result<Option<String>> {
    let target = target(reference)?;
    let mut ptr = std::ptr::null_mut();
    // SAFETY: target is NUL terminated; Windows allocates the credential on success.
    if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut ptr) } == 0 {
        let e = io::Error::last_os_error();
        return if e.raw_os_error() == Some(ERROR_NOT_FOUND as i32) { Ok(None) } else { Err(e) };
    }
    // SAFETY: successful CredReadW returns a valid CREDENTIALW and blob; copy before freeing it.
    let value = unsafe {
        let credential = &*ptr;
        if credential.CredentialBlobSize == 0 {
            Ok(String::new())
        } else if credential.CredentialBlobSize > 2560 || credential.CredentialBlob.is_null() {
            Err(io::Error::new(io::ErrorKind::InvalidData, "invalid stream credential blob"))
        } else {
            let bytes = std::slice::from_raw_parts(credential.CredentialBlob, credential.CredentialBlobSize as usize);
            String::from_utf8(bytes.to_vec())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid stream credential encoding"))
        }
    };
    // SAFETY: ptr belongs to CredReadW and must be released exactly once with CredFree.
    unsafe { CredFree(ptr.cast()) };
    value.map(Some)
}
pub fn save(reference: &str, key: &str) -> io::Result<()> {
    let target = target(reference)?;
    if key.is_empty() || key.len() > 2560 || key.chars().any(char::is_control) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid stream key"));
    }
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_ptr().cast_mut(),
        CredentialBlobSize: key.len() as u32,
        CredentialBlob: key.as_ptr().cast_mut(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..Default::default()
    };
    // SAFETY: the target and blob remain alive for the synchronous call; Windows copies both.
    if unsafe { CredWriteW(&credential, 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restricts_credential_namespace() {
        assert!(target("unrelated-account").is_err());
        assert!(target("CaptureStudio/stream/key\0suffix").is_err());
        assert!(target(&new_reference()).is_ok());
    }

    #[test]
    #[ignore = "writes and removes a synthetic credential in the current Windows account"]
    fn credential_manager_roundtrip() {
        let reference = new_reference();
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let name = target(&self.0).expect("test target");
                // SAFETY: deleting only the unique synthetic credential created by this test.
                unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) };
            }
        }
        let _cleanup = Cleanup(reference.clone());
        assert!(load(&reference).expect("missing credential").is_none());
        save(&reference, "synthetic-stream-key").expect("save");
        assert_eq!(load(&reference).expect("load").as_deref(), Some("synthetic-stream-key"));
    }
}
