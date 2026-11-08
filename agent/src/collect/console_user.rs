//! Get the currently logged-in user on the physical console (or any active
//! session). Works from a SYSTEM context where WMI's
//! Win32_ComputerSystem.UserName fails.

use windows::core::PWSTR;
use windows::Win32::System::RemoteDesktop::{
    WTSEnumerateSessionsW, WTSFreeMemory, WTSGetActiveConsoleSessionId,
    WTSQuerySessionInformationW, WTSActive, WTSDomainName, WTSUserName,
    WTS_CURRENT_SERVER_HANDLE, WTS_INFO_CLASS, WTS_SESSION_INFOW,
};

pub fn current_console_user() -> Option<String> {
    // 1) Active console session (physical screen)
    let console_sid = unsafe { WTSGetActiveConsoleSessionId() };
    if console_sid != u32::MAX {
        if let Some(u) = session_user(console_sid) {
            return Some(u);
        }
    }

    // 2) Any active session (RDP, VNC, etc.)
    unsafe {
        let mut sessions: *mut WTS_SESSION_INFOW = std::ptr::null_mut();
        let mut count: u32 = 0;

        if WTSEnumerateSessionsW(
            WTS_CURRENT_SERVER_HANDLE,
            0,
            1,
            &mut sessions,
            &mut count,
        )
        .is_err()
            || sessions.is_null()
        {
            return None;
        }

        let slice = std::slice::from_raw_parts(sessions, count as usize);
        let mut found = None;
        for s in slice {
            if s.State == WTSActive {
                if let Some(u) = session_user(s.SessionId) {
                    found = Some(u);
                    break;
                }
            }
        }
        WTSFreeMemory(sessions as *mut _);
        found
    }
}

fn session_user(session_id: u32) -> Option<String> {
    let user = query_string(session_id, WTSUserName)?;
    // Guard against pseudo-accounts (should not appear for active sessions,
    // but cheap insurance).
    if user.eq_ignore_ascii_case("SYSTEM")
        || user.eq_ignore_ascii_case("LOCAL SERVICE")
        || user.eq_ignore_ascii_case("NETWORK SERVICE")
    {
        return None;
    }
    let domain = query_string(session_id, WTSDomainName).unwrap_or_default();
    if domain.is_empty() {
        Some(user)
    } else {
        Some(format!("{}\\{}", domain, user))
    }
}

fn query_string(session_id: u32, class: WTS_INFO_CLASS) -> Option<String> {
    let mut buf = PWSTR::null();
    let mut bytes: u32 = 0;

    let ok = unsafe {
        WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            session_id,
            class,
            &mut buf,
            &mut bytes,
        )
    };

    if ok.is_err() || buf.is_null() || bytes <= 2 {
        if !buf.is_null() {
            unsafe { WTSFreeMemory(buf.0 as *mut _) };
        }
        return None;
    }

    let s = unsafe { buf.to_string().ok() };
    unsafe { WTSFreeMemory(buf.0 as *mut _) };
    s.filter(|v| !v.is_empty())
}
