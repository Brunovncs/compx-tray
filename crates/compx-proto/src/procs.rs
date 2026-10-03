//! Running and foreground processes, used by automatic profile switching.
//!
//! Names are handed out as borrowed UTF-16 slices so the periodic check allocates nothing.

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, MAX_PATH};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

fn until_nul(s: &[u16]) -> &[u16] {
    &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())]
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

/// Calls `f` with each running exe name; `f` returns `false` to stop early.
pub fn for_each_exe(mut f: impl FnMut(&[u16]) -> bool) {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return;
        }
        let _guard = Handle(snap);
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut entry) != 0;
        while ok {
            if !f(until_nul(&entry.szExeFile)) {
                return;
            }
            ok = Process32NextW(snap, &mut entry) != 0;
        }
    }
}

/// Lowercase `.exe` names currently running, sorted and deduplicated (for the rule picker).
pub fn running_exes() -> Vec<String> {
    let mut names = Vec::new();
    for_each_exe(|name| {
        let name = String::from_utf16_lossy(name).to_lowercase();
        if name.ends_with(".exe") {
            names.push(name);
        }
        true
    });
    names.sort_unstable();
    names.dedup();
    names
}

/// The official app; while it runs, this app keeps its hands off the mouse.
pub const ATK_HUB_EXE: &str = "atk hub.exe";

pub struct Scan {
    /// Profile of the earliest rule whose program is running.
    pub target: Option<u8>,
    pub atk_hub: bool,
}

/// One pass over the process list for both the rules and ATK HUB.
pub fn scan(rules: &[crate::rules::Rule]) -> Scan {
    let mut best: Option<usize> = None;
    let mut atk_hub = false;
    for_each_exe(|name| {
        if let Some(i) = crate::rules::rule_for(rules, name) {
            best = Some(best.map_or(i, |b| b.min(i)));
        }
        atk_hub |= crate::rules::exe_matches(name, ATK_HUB_EXE);
        true
    });
    Scan { target: best.map(|i| rules[i].profile), atk_hub }
}

/// Lowercase exe name of the process owning the foreground window.
pub fn foreground_exe() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let process = Handle(process);
        let mut buf = [0u16; MAX_PATH as usize * 2];
        let mut len = buf.len() as u32;
        if QueryFullProcessImageNameW(process.0, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) == 0 {
            return None;
        }
        let path = &buf[..len as usize];
        let file = path.rsplit(|&c| c == u16::from(b'\\')).next()?;
        Some(String::from_utf16_lossy(file).to_lowercase())
    }
}
