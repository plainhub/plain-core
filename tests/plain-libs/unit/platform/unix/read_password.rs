use super::*;

#[test]
fn hidden_input_supports_unicode_backspace_and_both_line_endings() {
    assert_eq!(
        read_line(&mut "passé\x7fword\n".as_bytes()).unwrap(),
        "password"
    );
    assert_eq!(read_line(&mut &b"value\r"[..]).unwrap(), "value");
    assert_eq!(read_line(&mut &b"\x08value\n"[..]).unwrap(), "value");
}

#[test]
fn hidden_input_rejects_cancellation_truncation_invalid_encoding_and_oversize() {
    for value in [3, 4, 26] {
        assert_eq!(
            read_line(&mut &[value][..]).unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
    }
    assert_eq!(
        read_line(&mut &b"unfinished"[..]).unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
    assert_eq!(
        read_line(&mut &[255, 10][..]).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(
        read_line(&mut vec![b'x'; 1025].as_slice())
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
}

#[test]
fn terminal_mode_disables_echo_and_restores_all_original_flags() {
    use std::os::fd::FromRawFd;
    let mut master = -1;
    let mut slave = -1;
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    let _master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    let read = || {
        let mut mode = unsafe { std::mem::zeroed::<libc::termios>() };
        assert_eq!(unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut mode) }, 0);
        mode
    };
    let before = read();
    let hidden = TerminalMode::hidden(&slave).unwrap();
    assert_eq!(
        read().c_lflag & (libc::ECHO | libc::ECHONL | libc::ICANON | libc::ISIG),
        0
    );
    drop(hidden);
    let after = read();
    assert_eq!(after.c_lflag, before.c_lflag);
    assert_eq!(after.c_iflag, before.c_iflag);
    assert_eq!(after.c_oflag, before.c_oflag);
    assert_eq!(after.c_cflag, before.c_cflag);
    assert_eq!(after.c_cc, before.c_cc);
}
