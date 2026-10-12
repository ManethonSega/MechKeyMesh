#[cfg(windows)]
use std::path::Path;

#[cfg(windows)]
pub fn executable_name() -> Option<String> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    };

    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return None;
        }
        let mut process_id = 0u32;
        GetWindowThreadProcessId(window, &mut process_id);
        if process_id == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if process.is_null() {
            return None;
        }
        let mut buffer = vec![0u16; 32_768];
        let mut length = buffer.len() as u32;
        let success = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length);
        CloseHandle(process);
        if success == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buffer[..length as usize]);
        Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
    }
}

#[cfg(not(windows))]
pub fn executable_name() -> Option<String> {
    None
}

pub fn pattern_matches(pattern: &str, executable: &str) -> bool {
    let pattern = pattern.trim().to_ascii_lowercase();
    let executable = executable.to_ascii_lowercase();
    if pattern.is_empty() {
        return false;
    }
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return pattern == executable;
    }
    let mut remainder = executable.as_str();
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if index == 0 && !pattern.starts_with('*') {
            let Some(next) = remainder.strip_prefix(part) else {
                return false;
            };
            remainder = next;
            continue;
        }
        let Some(position) = remainder.find(part) else {
            return false;
        };
        remainder = &remainder[position + part.len()..];
    }
    pattern.ends_with('*') || remainder.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_patterns_are_case_insensitive_and_support_wildcards() {
        assert!(pattern_matches("Photoshop.exe", "PHOTOSHOP.EXE"));
        assert!(pattern_matches("game*.exe", "game-win64.exe"));
        assert!(!pattern_matches("game.exe", "other.exe"));
    }
}
