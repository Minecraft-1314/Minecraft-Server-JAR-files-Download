use std::path::{Path, PathBuf};

use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

use super::input_validator::normalize_directory;

pub fn available_bytes(path: &str) -> i64 {
    let mut probe = match normalize_directory(path) {
        Ok(value) => PathBuf::from(value),
        Err(_) => return 0,
    };
    while !probe.is_dir() {
        match probe.parent() {
            Some(parent) if parent != Path::new("") => probe = parent.to_path_buf(),
            _ => break,
        }
    }
    free_space(&probe)
}

fn free_space(path: &Path) -> i64 {
    let mut wide: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut available: u64 = 0;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_mut_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return 0;
    }
    available as i64
}
