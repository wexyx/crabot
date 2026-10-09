use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    process::Stdio,
};
use tokio::io::{Interest, unix::AsyncFd};

/// Unix PTY with async nonblocking master I/O and an explicit controlling terminal.
pub(super) struct Pty {
    master: AsyncFd<OwnedFd>,
}
impl Pty {
    pub(super) fn attach(command: &mut tokio::process::Command) -> io::Result<Self> {
        let (mut master, mut slave) = (-1, -1);
        let mut size = libc::winsize {
            ws_row: 30,
            ws_col: 120,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        if unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        let master = unsafe { OwnedFd::from_raw_fd(master) };
        let slave = unsafe { OwnedFd::from_raw_fd(slave) };
        for fd in [master.as_raw_fd(), slave.as_raw_fd()] {
            if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        if unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut term = std::mem::MaybeUninit::uninit();
        if unsafe { libc::tcgetattr(slave.as_raw_fd(), term.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut term = unsafe { term.assume_init() };
        term.c_lflag &= !(libc::ECHO | libc::ECHONL);
        if unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &term) } != 0 {
            return Err(io::Error::last_os_error());
        }
        command
            .stdin(Stdio::from(slave.try_clone()?))
            .stdout(Stdio::from(slave.try_clone()?))
            .stderr(Stdio::from(slave));
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        Ok(Self {
            master: AsyncFd::new(master)?,
        })
    }
    pub(super) async fn read(&self, bytes: &mut [u8]) -> io::Result<usize> {
        let result = self
            .master
            .async_io(Interest::READABLE, |fd| {
                let n =
                    unsafe { libc::read(fd.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len()) };
                if n < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(n as usize)
                }
            })
            .await;
        match result {
            Err(e) if e.raw_os_error() == Some(libc::EIO) => Ok(0),
            other => other,
        }
    }
    pub(super) fn stop_foreground(&self) {
        // Job-control shells may put a foreground job in a different process group.
        let group = unsafe { libc::tcgetpgrp(self.master.get_ref().as_raw_fd()) };
        if group > 1 && group != unsafe { libc::getpgrp() } {
            unsafe {
                libc::kill(-group, libc::SIGKILL);
            }
        }
    }
    pub(super) async fn write(&self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            let n = self
                .master
                .async_io(Interest::WRITABLE, |fd| {
                    let n =
                        unsafe { libc::write(fd.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
                    if n < 0 {
                        Err(io::Error::last_os_error())
                    } else {
                        Ok(n as usize)
                    }
                })
                .await?;
            if n == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            bytes = &bytes[n..];
        }
        Ok(())
    }
}
