use std::ffi::CString;
use std::io;

#[derive(Debug)]
pub struct UserPrivileges {
    uid: u32,
    gid: u32,
}

impl UserPrivileges {
    #[cfg(unix)]
    pub fn new(username: &str) -> io::Result<Self> {
        let c_username = CString::new(username)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        unsafe {
            let pw = libc::getpwnam(c_username.as_ptr());
            if pw.is_null() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Error: could not find user \"{}\"", username),
                ));
            }

            Ok(Self {
                uid: (*pw).pw_uid,
                gid: (*pw).pw_gid,
            })
        }
    }

    #[cfg(windows)]
    pub fn new(username: &str) -> io::Result<Self> {
        // On Windows use fixed values
        // In real application Windows API can be used here
        Ok(Self {
            uid: 1000, // Default user ID
            gid: 1000, // Default group ID
        })
    }

    #[cfg(unix)]
    pub fn drop_privileges(&self) -> io::Result<()> {
        unsafe {
            if libc::setgid(self.gid) != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Error: failed to set group"
                ));
            }

            if libc::setuid(self.uid) != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Error: failed to set user"
                ));
            }

            Ok(())
        }
    }

    #[cfg(windows)]
    pub fn drop_privileges(&self) -> io::Result<()> {
        // On Windows drop privileges is not supported in this form
        // Just return Ok
        Ok(())
    }

    #[cfg(unix)]
    pub fn check_root() -> io::Result<()> {
        unsafe {
            if libc::getuid() != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Error: must be root to use option -u"
                ));
            }
            Ok(())
        }
    }

    #[cfg(windows)]
    pub fn check_root() -> io::Result<()> {
        // On Windows check administrator rights
        // For simplicity always return Ok
        Ok(())
    }
}