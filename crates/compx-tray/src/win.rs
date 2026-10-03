//! Win32 plumbing on the UI thread: a hidden window that receives system broadcasts,
//! the foreground-window hook, global hotkeys and the profile OSD.
//!
//! Everything reports back as thread messages, so the main loop handles all input the same way.

use std::cell::RefCell;

use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint,
    FillRect, GetDC, GetTextExtentPoint32W, InvalidateRect, ReleaseDC, SelectObject, SetBkMode,
    SetTextColor, SetWindowRgn, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH,
    DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_VCENTER, FW_SEMIBOLD, HDC, HFONT, OUT_DEFAULT_PRECIS,
    PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, KillTimer, PostThreadMessageW, RegisterClassW,
    SetLayeredWindowAttributes, SetTimer, SetWindowPos, ShowWindow, SystemParametersInfoW,
    EVENT_SYSTEM_FOREGROUND, HWND_TOPMOST, LWA_ALPHA, SPI_GETWORKAREA, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SW_HIDE, WINEVENT_OUTOFCONTEXT, WM_APP, WM_DEVICECHANGE, WM_PAINT, WM_SETTINGCHANGE, WM_TIMER,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP,
};

pub const WM_DEVICES_CHANGED: u32 = WM_APP + 3;
pub const WM_THEME_CHANGED: u32 = WM_APP + 4;
pub const WM_FOREGROUND_CHANGED: u32 = WM_APP + 5;

/// Hotkey ids are `1..=profiles`; `WM_HOTKEY`'s wParam minus one is the profile index.
pub const HOTKEY_MODS: u32 = MOD_CONTROL | MOD_ALT | MOD_SHIFT;

const DBT_DEVNODES_CHANGED: WPARAM = 0x0007;
const OSD_TIMER: usize = 1;
const OSD_MS: u32 = 1400;
const OSD_DARK: [u8; 3] = [0x20, 0x22, 0x27];
const OSD_DARK_TEXT: [u8; 3] = [0xEC, 0xEE, 0xF1];
const OSD_LIGHT: [u8; 3] = [0xFF, 0xFF, 0xFF];
const OSD_LIGHT_TEXT: [u8; 3] = [0x2B, 0x2D, 0x33];

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn rgb([r, g, b]: [u8; 3]) -> COLORREF {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}

fn post_self(msg: u32) {
    unsafe { PostThreadMessageW(GetCurrentThreadId(), msg, 0, 0) };
}

fn scale(px: i32) -> i32 {
    px * unsafe { GetDpiForSystem() } as i32 / 96
}

unsafe fn wide_eq(ptr: *const u16, s: &str) -> bool {
    s.encode_utf16().chain(Some(0)).enumerate().all(|(i, c)| *ptr.add(i) == c)
}

unsafe extern "system" fn events_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_DEVICECHANGE if wp == DBT_DEVNODES_CHANGED => post_self(WM_DEVICES_CHANGED),
        WM_SETTINGCHANGE if lp != 0 && wide_eq(lp as *const u16, "ImmersiveColorSet") => {
            post_self(WM_THEME_CHANGED)
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

unsafe extern "system" fn on_foreground(
    _: HWINEVENTHOOK,
    _: u32,
    _: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    post_self(WM_FOREGROUND_CHANGED);
}

/// What the OSD shows; set before each repaint.
struct Osd {
    text: Vec<u16>,
    dot: [u8; 3],
    dark: bool,
}

thread_local! {
    static OSD: RefCell<Osd> = const { RefCell::new(Osd { text: Vec::new(), dot: [0; 3], dark: true }) };
}

const DOT: [u16; 1] = [0x25CF]; // ●, drawn as text so it gets anti-aliased like the label

fn font(px: i32) -> HFONT {
    let face = wide("Segoe UI Variable Text");
    unsafe {
        CreateFontW(
            -scale(px),
            0,
            0,
            0,
            FW_SEMIBOLD as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32,
            face.as_ptr(),
        )
    }
}

/// Width of `text` in `hfont`, in physical pixels.
unsafe fn text_width(hdc: HDC, hfont: HFONT, text: &[u16]) -> i32 {
    let old = SelectObject(hdc, hfont);
    let mut size: SIZE = std::mem::zeroed();
    GetTextExtentPoint32W(hdc, text.as_ptr(), text.len() as i32, &mut size);
    SelectObject(hdc, old);
    size.cx
}

const OSD_PAD: i32 = 14;
const OSD_GAP: i32 = 8;
const OSD_HEIGHT: i32 = 36;
const OSD_MARGIN: i32 = 16;
const OSD_TEXT_PX: i32 = 13;
const OSD_DOT_PX: i32 = 11;

unsafe extern "system" fn osd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            OSD.with_borrow(|osd| {
                let (bg, fg) = if osd.dark { (OSD_DARK, OSD_DARK_TEXT) } else { (OSD_LIGHT, OSD_LIGHT_TEXT) };
                let brush = CreateSolidBrush(rgb(bg));
                FillRect(hdc, &rc, brush);
                DeleteObject(brush);
                SetBkMode(hdc, TRANSPARENT as i32);

                let (dot_font, text_font) = (font(OSD_DOT_PX), font(OSD_TEXT_PX));
                let mut dot_rc = RECT { left: scale(OSD_PAD), ..rc };
                SetTextColor(hdc, rgb(osd.dot));
                let old = SelectObject(hdc, dot_font);
                DrawTextW(hdc, DOT.as_ptr(), 1, &mut dot_rc, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
                let dot_w = text_width(hdc, dot_font, &DOT);

                let mut text_rc = RECT { left: dot_rc.left + dot_w + scale(OSD_GAP), right: rc.right - scale(OSD_PAD), ..rc };
                SetTextColor(hdc, rgb(fg));
                SelectObject(hdc, text_font);
                DrawTextW(
                    hdc,
                    osd.text.as_ptr(),
                    osd.text.len() as i32,
                    &mut text_rc,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                );
                SelectObject(hdc, old);
                DeleteObject(dot_font);
                DeleteObject(text_font);
            });
            EndPaint(hwnd, &ps);
            0
        }
        WM_TIMER => {
            KillTimer(hwnd, OSD_TIMER);
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn apps_use_dark_theme() -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|k| k.get_value::<u32, _>("AppsUseLightTheme"))
        .map_or(true, |v| v == 0)
}

unsafe fn create(class: &str, proc_: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT, ex: u32) -> HWND {
    let instance = GetModuleHandleW(std::ptr::null());
    let name = wide(class);
    let wc = WNDCLASSW {
        lpfnWndProc: Some(proc_),
        hInstance: instance,
        lpszClassName: name.as_ptr(),
        ..std::mem::zeroed()
    };
    RegisterClassW(&wc);
    CreateWindowExW(
        ex,
        name.as_ptr(),
        name.as_ptr(),
        WS_POPUP,
        0,
        0,
        0,
        0,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        instance,
        std::ptr::null(),
    )
}

pub struct Win {
    osd: HWND,
    hook: HWINEVENTHOOK,
    hotkeys: u8,
}

impl Win {
    /// Must run on the thread that owns the message loop.
    pub fn new() -> Self {
        unsafe {
            // Hidden top-level window: message-only windows do not receive broadcasts.
            create("compx-tray-events", events_proc, WS_EX_TOOLWINDOW);
            let osd = create(
                "compx-tray-osd",
                osd_proc,
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            );
            SetLayeredWindowAttributes(osd, 0, 245, LWA_ALPHA);
            Self { osd, hook: std::ptr::null_mut(), hotkeys: 0 }
        }
    }

    pub fn set_foreground_hook(&mut self, on: bool) {
        unsafe {
            if on && self.hook.is_null() {
                self.hook = SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    std::ptr::null_mut(),
                    Some(on_foreground),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
            } else if !on && !self.hook.is_null() {
                UnhookWinEvent(self.hook);
                self.hook = std::ptr::null_mut();
            }
        }
    }

    /// Ctrl+Alt+Shift+1…`count`; 0 turns them off. Keys taken by other apps are skipped.
    pub fn set_hotkeys(&mut self, count: u8) {
        if count == self.hotkeys {
            return;
        }
        unsafe {
            for id in 1..=self.hotkeys {
                UnregisterHotKey(std::ptr::null_mut(), id as i32);
            }
            for id in 1..=count {
                RegisterHotKey(std::ptr::null_mut(), id as i32, HOTKEY_MODS | MOD_NOREPEAT, (b'0' + id) as u32);
            }
        }
        self.hotkeys = count;
    }

    /// Small click-through toast in the bottom-right corner, above the taskbar.
    pub fn show_osd(&self, text: &str, dot: [u8; 3]) {
        let text: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            let hdc = GetDC(self.osd);
            let (dot_font, text_font) = (font(OSD_DOT_PX), font(OSD_TEXT_PX));
            let content = text_width(hdc, dot_font, &DOT) + scale(OSD_GAP) + text_width(hdc, text_font, &text);
            DeleteObject(dot_font);
            DeleteObject(text_font);
            ReleaseDC(self.osd, hdc);

            let w = (content + scale(OSD_PAD) * 2).clamp(scale(120), scale(280));
            let h = scale(OSD_HEIGHT);
            let mut area: RECT = std::mem::zeroed();
            SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut area as *mut RECT).cast(), 0);
            let x = area.right - w - scale(OSD_MARGIN);
            let y = area.bottom - h - scale(OSD_MARGIN);

            OSD.with_borrow_mut(|o| *o = Osd { text, dot, dark: apps_use_dark_theme() });
            SetWindowRgn(self.osd, CreateRoundRectRgn(0, 0, w + 1, h + 1, scale(14), scale(14)), 0);
            SetWindowPos(self.osd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            InvalidateRect(self.osd, std::ptr::null(), 1);
            SetTimer(self.osd, OSD_TIMER, OSD_MS, None);
        }
    }
}
