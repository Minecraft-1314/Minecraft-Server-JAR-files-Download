use crate::core::config_store;
use crate::core::error::{AppError, AppResult};
use crate::core::url_guard::is_public_url;

pub fn open(value: &str) -> AppResult<()> {
    if !is_public_url(value) {
        return Err(AppError::validation("url.blocked"));
    }
    open_target(value)
}

pub fn open_data_directory() -> AppResult<()> {
    let directory = config_store::data_directory();
    std::fs::create_dir_all(&directory)?;
    open_target(&directory.to_string_lossy())
}

fn open_target(value: &str) -> AppResult<()> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let operation: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
    let target: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        return Err(AppError::general("shell.openFailed"));
    }
    Ok(())
}
