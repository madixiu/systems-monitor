use anyhow::{Context, Result};
use windows::core::PWSTR;
use windows::Win32::System::RemoteDesktop::{
    WTSEnumerateSessionsW, WTSFreeMemory, WTSGetActiveConsoleSessionId,
    WTSQuerySessionInformationW, WTSActive, WTSDisconnected, WTSDomainName, WTSUserName,
    WTS_CURRENT_SERVER_HANDLE, WTS_INFO_CLASS, WTS_SESSION_INFOW,
};

pub fn current_console_user() -> Result<Option<String>> {
    let console_sid = unsafe { WTSGetActiveConsoleSessionId() };
    let mut sessions: *mut WTS_SESSION_INFOW = std::ptr::null_mut();
    let mut count = 0;
    unsafe {
        WTSEnumerateSessionsW(WTS_CURRENT_SERVER_HANDLE, 0, 1, &mut sessions, &mut count)
            .context("enumerate Windows sessions")?;
    }

    let mut candidates = Vec::new();
    if !sessions.is_null() {
        let slice = unsafe { std::slice::from_raw_parts(sessions, count as usize) };
        candidates.extend(slice.iter()
            .filter(|s| s.State == WTSActive || s.State == WTSDisconnected)
            .map(|s| (s.SessionId, s.State)));
        unsafe { WTSFreeMemory(sessions as *mut _) };
    } else if count != 0 {
        anyhow::bail!("Windows session enumeration returned a null buffer");
    }
    candidates.sort_by_key(|(sid, state)| {
        if *state == WTSActive && *sid == console_sid { 0 }
        else if *state == WTSActive { 1 }
        else { 2 }
    });
    for (session_id, _) in candidates {
        if let Some(user) = session_user(session_id)? {
            return Ok(Some(user));
        }
    }
    Ok(None)
}

fn session_user(session_id: u32) -> Result<Option<String>> {
    let Some(user) = query_string(session_id, WTSUserName)? else {
        return Ok(None);
    };
    if ["SYSTEM", "LOCAL SERVICE", "NETWORK SERVICE"]
        .iter()
        .any(|name| user.eq_ignore_ascii_case(name))
    {
        return Ok(None);
    }
    let domain = query_string(session_id, WTSDomainName)?;
    Ok(Some(match domain {
        Some(domain) => format!("{domain}\\{user}"),
        None => user,
    }))
}

fn query_string(session_id: u32, class: WTS_INFO_CLASS) -> Result<Option<String>> {
    let mut buf = PWSTR::null();
    let mut bytes = 0;
    let result = unsafe {
        WTSQuerySessionInformationW(WTS_CURRENT_SERVER_HANDLE, session_id, class, &mut buf, &mut bytes)
    };
    if let Err(error) = result {
        if !buf.is_null() {
            unsafe { WTSFreeMemory(buf.0 as *mut _) };
        }
        return Err(error).context("query Windows session information");
    }
    if buf.is_null() || bytes <= 2 {
        if !buf.is_null() {
            unsafe { WTSFreeMemory(buf.0 as *mut _) };
        }
        return Ok(None);
    }
    let value = unsafe { buf.to_string() };
    unsafe { WTSFreeMemory(buf.0 as *mut _) };
    Ok(Some(value.context("decode Windows session information")?).filter(|s| !s.is_empty()))
}
