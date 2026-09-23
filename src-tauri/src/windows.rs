//! Windows-only adapter and process ownership helpers.
use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub fn verified_runtime(path: &Path, hash: &str) -> Result<()> {
    ensure!(
        path.is_file(),
        "Не найден компонент {}. Распакуйте ZIP целиком",
        path.file_name().unwrap_or_default().to_string_lossy()
    );
    ensure!(
        path.metadata()?.len() <= 150 * 1024 * 1024,
        "Некорректный размер компонента"
    );
    let digest = format!("{:x}", Sha256::digest(std::fs::read(path)?));
    ensure!(
        digest == hash,
        "Контрольная сумма компонента не совпала; запуск отменён"
    );
    Ok(())
}
pub fn hidden(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}
pub fn admin() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        };
        unsafe {
            let mut handle = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) == 0 {
                return false;
            }
            let mut info: TOKEN_ELEVATION = std::mem::zeroed();
            let mut len = 0;
            let ok = GetTokenInformation(
                handle,
                TokenElevation,
                (&mut info as *mut TOKEN_ELEVATION).cast(),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut len,
            );
            CloseHandle(handle);
            ok != 0 && info.TokenIsElevated != 0
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}
pub fn community_url(destination: &str) -> Result<&'static str> {
    match destination {
        "telegram" => Ok("https://t.me/VenoXiss"),
        "github" => Ok("https://github.com/vpnxis"),
        _ => anyhow::bail!("Неизвестная ссылка команды"),
    }
}
fn clipboard_decode(units: &[u16]) -> Result<String> {
    let end = units
        .iter()
        .position(|v| *v == 0)
        .context("Некорректный текст в буфере")?;
    let text = String::from_utf16(&units[..end]).context("Буфер не содержит корректный Unicode")?;
    ensure!(text.len() <= 262144, "Буфер больше 256 КиБ");
    Ok(text)
}
pub fn clipboard_text() -> Result<String> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::{
            DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard},
            Memory::{GlobalLock, GlobalSize, GlobalUnlock},
        };
        ensure!(
            OpenClipboard(std::ptr::null_mut()) != 0,
            "Буфер обмена занят другим приложением. Попробуйте ещё раз"
        );
        struct Clipboard;
        impl Drop for Clipboard {
            fn drop(&mut self) {
                unsafe {
                    CloseClipboard();
                }
            }
        }
        let _clipboard = Clipboard;
        let handle = GetClipboardData(13); // CF_UNICODETEXT; read-only, never empty/set the clipboard.
        if handle.is_null() {
            return Ok(String::new());
        }
        let bytes = GlobalSize(handle);
        ensure!(
            (2..=524290).contains(&bytes) && bytes % 2 == 0,
            "Некорректный размер текста в буфере"
        );
        let ptr = GlobalLock(handle);
        ensure!(!ptr.is_null(), "Не удалось прочитать буфер обмена");
        let result = clipboard_decode(std::slice::from_raw_parts(ptr.cast::<u16>(), bytes / 2));
        GlobalUnlock(handle);
        result
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Только Windows")
    }
}
pub fn open_community(destination: &str) -> Result<()> {
    shell_open(community_url(destination)?, false)
}
pub fn relaunch_elevated() -> Result<()> {
    ensure!(!admin(), "Приложение уже запущено от администратора");
    shell_open(&std::env::current_exe()?.to_string_lossy(), true)
}
fn shell_open(target: &str, elevate: bool) -> Result<()> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::{
            Shell::{ShellExecuteExW, SHELLEXECUTEINFOW},
            WindowsAndMessaging::SW_SHOWNORMAL,
        };
        let verb: Vec<u16> = if elevate { "runas" } else { "open" }
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let path: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of_val(&info) as u32;
        info.lpVerb = verb.as_ptr();
        info.lpFile = path.as_ptr();
        info.nShow = SW_SHOWNORMAL;
        if ShellExecuteExW(&mut info) == 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(1223) {
                anyhow::bail!("Запуск от администратора отменён. Подключение не запускалось")
            }
            anyhow::bail!("Windows не смогла открыть приложение или браузер");
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (target, elevate);
        anyhow::bail!("Только Windows")
    }
}

#[cfg(test)]
mod link_tests {
    #[test]
    fn clipboard_handles_large_unicode_without_shell_or_truncation() {
        let value = "Сервер 🌍\r\n".repeat(4000);
        let units: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
        assert_eq!(super::clipboard_decode(&units).unwrap(), value);
        assert!(super::clipboard_decode(&[0xd800, 0]).is_err());
        assert!(super::clipboard_decode(&[1, 2]).is_err());
        assert!(super::clipboard_decode(&vec![65; 262146]).is_err());
    }
    #[test]
    fn community_links_are_not_an_arbitrary_shell_opener() {
        assert_eq!(
            super::community_url("telegram").unwrap(),
            "https://t.me/VenoXiss"
        );
        assert_eq!(
            super::community_url("github").unwrap(),
            "https://github.com/vpnxis"
        );
        for input in ["cmd.exe", "file:///C:/", "https://evil.invalid", ""] {
            assert!(super::community_url(input).is_err());
        }
    }
}
pub fn powershell(script: &str, payload: &serde_json::Value) -> Result<String> {
    // No interpolated shell values. Data is a separate environment value, parsed as JSON.
    let mut c = Command::new("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe");
    hidden(&mut c)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .env("VXSSTUN_TASK_JSON", serde_json::to_string(payload)?)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let mut child = c
        .spawn()
        .context("Не удалось запустить системную настройку сети")?;
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("Windows не завершила настройку сети за 25 секунд")
        };
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output()?;
    ensure!(
        out.status.success(),
        "Windows отклонила настройку сети. Для TUN нужны права администратора"
    );
    ensure!(out.stdout.len() <= 32768, "Некорректный ответ Windows");
    Ok(String::from_utf8_lossy(&out.stdout)
        .trim_start_matches('\u{feff}')
        .trim()
        .into())
}

pub struct OwnedProcess {
    pub child: Child,
    #[cfg(windows)]
    job: windows_sys::Win32::Foundation::HANDLE,
}
unsafe impl Send for OwnedProcess {}
impl OwnedProcess {
    pub fn spawn(exe: &Path, config: &[u8], cwd: &Path) -> Result<Self> {
        let mut c = Command::new(exe);
        hidden(&mut c)
            .args(["run", "-config", "stdin:"])
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = c.spawn().context("Не удалось запустить Xray")?;
        let mut owned = Self {
            child,
            #[cfg(windows)]
            job: std::ptr::null_mut(),
        };
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::JobObjects::*;
            unsafe {
                owned.job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                ensure!(
                    !owned.job.is_null(),
                    "Не удалось создать контейнер процесса"
                );
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                ensure!(
                    SetInformationJobObject(
                        owned.job,
                        JobObjectExtendedLimitInformation,
                        (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                        std::mem::size_of_val(&limits) as u32
                    ) != 0,
                    "Не удалось ограничить дочерний процесс"
                );
                ensure!(
                    AssignProcessToJobObject(owned.job, owned.child.as_raw_handle()) != 0,
                    "Не удалось привязать Xray к приложению"
                );
            }
        }
        owned
            .child
            .stdin
            .take()
            .context("Нет входа процесса")?
            .write_all(config)?;
        Ok(owned)
    }
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        #[cfg(windows)]
        unsafe {
            if !self.job.is_null() {
                windows_sys::Win32::Foundation::CloseHandle(self.job);
            }
        }
    }
}
