use chrono::{Local, TimeZone};

pub fn format_date(ts: i64) -> String {
    if ts == 0 {
        return "-".into();
    }
    match Local.timestamp_opt(ts, 0).single() {
        Some(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        None => "-".into(),
    }
}

pub fn username(uid: u32) -> String {
    unsafe {
        let mut pwd: libc::passwd = std::mem::zeroed();
        let mut buf = [0 as libc::c_char; 1024];
        let mut result: *mut libc::passwd = std::ptr::null_mut();
        let rc = libc::getpwuid_r(
            uid as libc::uid_t,
            &mut pwd,
            buf.as_mut_ptr(),
            buf.len(),
            &mut result,
        );
        if rc == 0 && !result.is_null() {
            let name = std::ffi::CStr::from_ptr(pwd.pw_name);
            return name.to_string_lossy().into_owned();
        }
    }
    uid.to_string()
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    if bytes == 0 {
        return "0 B".into();
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn snapshot_type(kind: u8) -> &'static str {
    match kind {
        0 => "single",
        1 => "pre",
        2 => "post",
        _ => "unknown",
    }
}
