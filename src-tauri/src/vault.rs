use anyhow::{ensure, Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;

// DPAPI CurrentUser, UI forbidden. Neither a global machine key nor plaintext fallback.
#[cfg(windows)]
fn crypt(data: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    ensure!(data.len() <= 4 * 1024 * 1024, "Хранилище слишком большое");
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    ensure!(
        ok != 0,
        "Windows не смог открыть защищённое хранилище этого пользователя"
    );
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(result)
}
#[cfg(not(windows))]
fn crypt(_data: &[u8], _encrypt: bool) -> Result<Vec<u8>> {
    anyhow::bail!("Хранилище доступно только в Windows")
}

pub fn load<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    ensure!(
        path.metadata()?.len() <= 4 * 1024 * 1024,
        "Повреждено хранилище профилей"
    );
    Ok(Some(serde_json::from_slice(&crypt(
        &std::fs::read(path)?,
        false,
    )?)?))
}
pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let encrypted = crypt(&serde_json::to_vec(value)?, true)?;
    let parent = path.parent().context("Нет каталога данных")?;
    std::fs::create_dir_all(parent)?;
    // Same-volume atomic replacement, retain the previous valid file on failure.
    let temp = parent.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    file.write_all(&encrypted)?;
    file.sync_all()?;
    drop(file);
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(old: *const u16, new: *const u16, flags: u32) -> i32;
        }
        let from: Vec<_> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let ok = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0x1 | 0x8) };
        if ok == 0 {
            let _ = std::fs::remove_file(&temp);
            anyhow::bail!(
                "Не удалось сохранить хранилище: {}",
                std::io::Error::last_os_error()
            )
        }
    }
    #[cfg(not(windows))]
    std::fs::rename(temp, path)?;
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn dpapi_roundtrip_and_tamper() {
        let bytes = crypt(b"test-secret", true).unwrap();
        assert!(!bytes.windows(11).any(|w| w == b"test-secret"));
        assert_eq!(crypt(&bytes, false).unwrap(), b"test-secret");
        let mut b = bytes;
        b[20] ^= 1;
        assert!(crypt(&b, false).is_err());
    }
}
