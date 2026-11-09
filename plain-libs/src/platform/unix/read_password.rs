use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;

pub fn prompt_password(prompt: &str) -> io::Result<String> {
    let mut terminal = OpenOptions::new().read(true).write(true).open("/dev/tty")
        .map_err(|_| io::Error::other("An interactive terminal is required; use passwd --stdin to provision a password before installation"))?;
    let mode = TerminalMode::hidden(&terminal)?;
    terminal.write_all(prompt.as_bytes())?;
    terminal.flush()?;
    let result = read_line(&mut terminal);
    drop(mode);
    terminal.write_all(b"\n")?;
    result
}

fn read_line(terminal: &mut impl Read) -> io::Result<String> {
    let mut bytes = Vec::with_capacity(64);
    for _ in 0..4096 {
        let mut byte = [0];
        terminal.read_exact(&mut byte)?;
        match byte[0] {
            b'\r' | b'\n' => {
                return String::from_utf8(bytes)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
            }
            3 | 4 | 26 => {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Password entry cancelled",
                ));
            }
            8 | 127 => while bytes.pop().is_some_and(|byte| byte & 0xc0 == 0x80) {},
            value => {
                bytes.push(value);
                if bytes.len() > 1024 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Password must contain 1 to 1024 bytes",
                    ));
                }
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "Password entry is too long",
    ))
}

struct TerminalMode {
    terminal: File,
    original: libc::termios,
}

impl TerminalMode {
    fn hidden(terminal: &File) -> io::Result<Self> {
        let terminal = terminal.try_clone()?;
        let mut original = unsafe { std::mem::zeroed::<libc::termios>() };
        if unsafe { libc::tcgetattr(terminal.as_raw_fd(), &mut original) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut hidden = original;
        hidden.c_lflag &= !(libc::ECHO | libc::ECHONL | libc::ICANON | libc::ISIG);
        hidden.c_cc[libc::VMIN] = 1;
        hidden.c_cc[libc::VTIME] = 0;
        if unsafe { libc::tcsetattr(terminal.as_raw_fd(), libc::TCSAFLUSH, &hidden) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { terminal, original })
    }
}

impl Drop for TerminalMode {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(self.terminal.as_raw_fd(), libc::TCSAFLUSH, &self.original);
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/platform/unix/read_password.rs"]
mod tests;
