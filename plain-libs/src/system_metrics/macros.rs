macro_rules! sysinfo_debug { ($($message:tt)*) => { log::debug!($($message)*); }; }

#[cfg(unix)]

macro_rules! retry_eintr {
    (set_to_0 => $($t:tt)+) => {{
        let errno = crate::system_metrics::unix::libc_errno();
        if !errno.is_null() {
            *errno = 0;
        }
        retry_eintr!($($t)+)
    }};
    ($errno_value:ident => $($t:tt)+) => {{
        loop {
            let ret = $($t)+;
            if ret < 0 {
                let tmp = std::io::Error::last_os_error();
                if tmp.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                $errno_value = tmp.raw_os_error().unwrap_or(0);
            }
            break ret;
        }
    }};
    ($($t:tt)+) => {{
        loop {
            let ret = $($t)+;
            if ret < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            break ret;
        }
    }};
}
